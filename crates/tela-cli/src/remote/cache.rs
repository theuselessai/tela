//! Descriptor-relative cache IO: never follow symlinks, including in the root path.
use anyhow::{ensure, Context, Result};
use rustix::fs::{self, AtFlags, FileType, Mode, OFlags};
use rustix::io::Errno;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};

pub(super) struct Cache {
    directory: File,
}

fn private(file: &File, directory: bool) -> Result<()> {
    let stat = fs::fstat(file)?;
    let expected = if directory {
        FileType::Directory
    } else {
        FileType::RegularFile
    };
    ensure!(
        FileType::from_raw_mode(stat.st_mode) == expected,
        "unsafe cache file type"
    );
    ensure!(
        stat.st_uid == rustix::process::geteuid().as_raw(),
        "cache must be owned by current user"
    );
    ensure!(
        stat.st_mode & 0o077 == 0,
        "cache must be private (directory 0700, files 0600)"
    );
    if !directory {
        ensure!(stat.st_nlink == 1, "hard-linked cache file is forbidden");
    }
    Ok(())
}

fn key_valid(key: &str) -> Result<()> {
    ensure!(
        key.starts_with("bundle-") || key.starts_with("grant-"),
        "invalid cache key"
    );
    let digest = key.split_once('-').unwrap().1;
    ensure!(
        digest.len() == 64
            && digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid cache key"
    );
    Ok(())
}

impl Cache {
    pub(super) fn open(root: &Path) -> Result<Self> {
        ensure!(root.is_absolute(), "cache root must be absolute");
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let mut directory = File::from(fs::open("/", flags, Mode::empty())?);
        let components = root.components().collect::<Vec<_>>();
        ensure!(components.len() > 1, "invalid cache root");
        for component in components.into_iter().skip(1) {
            let Component::Normal(name) = component else {
                anyhow::bail!("unsafe cache root");
            };
            match fs::mkdirat(&directory, name, Mode::from_raw_mode(0o700)) {
                Ok(()) | Err(Errno::EXIST) => {}
                Err(error) => return Err(error.into()),
            }
            directory = File::from(
                fs::openat(&directory, name, flags, Mode::empty())
                    .context("cache path must contain only real directories (no symlinks)")?,
            );
        }
        private(&directory, true)?;
        Ok(Self { directory })
    }

    pub(super) fn read(&self, key: &str, limit: usize) -> Result<Option<Vec<u8>>> {
        key_valid(key)?;
        // NONBLOCK prevents malicious FIFOs from hanging before fstat rejects them.
        let fd = match fs::openat(
            &self.directory,
            key,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => return Ok(None),
            Err(error) => return Err(error).context("unsafe or unreadable cache entry"),
        };
        let file = File::from(fd);
        private(&file, false)?;
        ensure!(
            fs::fstat(&file)?.st_size >= 0 && fs::fstat(&file)?.st_size as u64 <= limit as u64,
            "cache entry exceeds size limit"
        );
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= limit, "cache entry exceeds size limit");
        Ok(Some(bytes))
    }

    pub(super) fn write(&self, key: &str, bytes: &[u8]) -> Result<()> {
        key_valid(key)?;
        // Validate an existing target as well. renameat would not follow it, but
        // rejecting unsafe targets makes tampering explicit rather than repairing it.
        self.read(key, super::BUNDLE_LIMIT)?;
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let (temporary, mut file) = loop {
            let name = format!(
                ".tmp-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            );
            match fs::openat(
                &self.directory,
                &name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            ) {
                Ok(fd) => break (name, File::from(fd)),
                Err(Errno::EXIST) => continue,
                Err(error) => return Err(error.into()),
            }
        };
        let result = (|| -> Result<()> {
            private(&file, false)?;
            file.write_all(bytes)?;
            file.sync_all()?;
            fs::renameat(&self.directory, &temporary, &self.directory, key)?;
            self.directory.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::unlinkat(&self.directory, &temporary, AtFlags::empty());
        }
        result
    }
}
