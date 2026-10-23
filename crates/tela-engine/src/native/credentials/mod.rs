//! Owner-only, app-scoped opaque credential files for the native-module bridge.

use crate::native::NativeModule;
use rand_core::{OsRng, RngCore};
use rquickjs::{Ctx, Exception, Result as JsResult};
use rustix::fs::{fsync, linkat, open, openat, unlinkat, AtFlags, Dir, Mode, OFlags};
use serde_json::{json, Value};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

const MAX_BYTES: usize = 16 * 1024;
const MAX_ID: usize = 64;
// Generic provider bound, not an application's profile limit. Includes temporary files.
const MAX_ENTRIES: usize = 128;
// Dropping a caller future does not stop a blocking filesystem operation.
// Keep admission in the worker so repeated shutdown cannot accumulate workers.
static FILE_OPERATIONS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);
const BOOTSTRAP: &str = r#"
(function () {
    globalThis.Tela.credentials = {
        prepare: function () { return Tela.native.call("credentials", "prepare", null); },
        list: function () { return Tela.native.call("credentials", "list", null); },
        load: function (name) { return Tela.native.call("credentials", "load", { name: name }); },
        saveNew: function (name, value) {
            return Tela.native.call("credentials", "saveNew", { name: name, value: value });
        },
        remove: function (name) { return Tela.native.call("credentials", "remove", { name: name }); },
    };
})();
"#;

/// Build the permission-gated `credentials` module for a host-derived app identity.
/// The identity must come from the trusted loader (for example, its URL hash), not
/// from an application manifest.
pub(crate) fn credentials_module(app_identity: &str) -> anyhow::Result<NativeModule> {
    let base = dirs_next::data_local_dir()
        .filter(|path| path.is_absolute())
        .ok_or_else(|| anyhow::anyhow!("credentials data directory unavailable"))?;
    module_at(app_identity, base.join("tela-credentials"))
}

/// Install the small JavaScript convenience surface after `Tela.native` exists.
pub(crate) fn register_credentials(ctx: &Ctx<'_>) -> JsResult<()> {
    if !ctx
        .globals()
        .get::<_, rquickjs::Object>("Tela")?
        .contains_key("native")?
    {
        return Err(Exception::throw_type(
            ctx,
            "credentials requires Tela.native",
        ));
    }
    ctx.eval::<(), _>(BOOTSTRAP)
}

pub(crate) fn module_at(
    app_identity: &str,
    provider_root: PathBuf,
) -> anyhow::Result<NativeModule> {
    valid_identifier(app_identity, 128).map_err(anyhow::Error::msg)?;
    if !provider_root.is_absolute() {
        anyhow::bail!("credentials root must be absolute");
    }
    let root = provider_root.join(app_identity);
    let provider_parent = provider_root
        .parent()
        .ok_or_else(|| anyhow::anyhow!("credentials root invalid"))?;
    let store = Arc::new(Store {
        trusted_base: Some(provider_parent.to_path_buf()),
        dirs: vec![provider_root, root.clone()],
        root,
    });
    Ok(NativeModule::new(
        "credentials",
        "credentials",
        move |operation, params, cancellation| {
            let store = store.clone();
            async move {
                if cancellation.is_cancelled() {
                    return Err("credentials unavailable".to_string());
                }
                let permit = FILE_OPERATIONS
                    .try_acquire()
                    .map_err(|_| "credentials busy".to_string())?;
                // The worker owns admission even if this async caller is cancelled.
                let result = tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    store.dispatch(&operation, params)
                })
                .await
                .map_err(|_| "credentials operation failed".to_string())?;
                if cancellation.is_cancelled() {
                    return Err("credentials unavailable; write outcome uncertain".to_string());
                }
                result
            }
        },
    ))
}

struct Store {
    trusted_base: Option<PathBuf>,
    // Only provider-owned path components are created or validated; the OS data root is trusted.
    dirs: Vec<PathBuf>,
    root: PathBuf,
}

impl Store {
    fn dispatch(&self, operation: &str, params: Value) -> Result<Value, String> {
        match operation {
            "prepare" => {
                self.prepare()?;
                Ok(Value::Null)
            }
            "list" => self.list(),
            "load" => self.load(name(&params)?),
            "saveNew" => self.save_new(
                name(&params)?,
                params.get("value").ok_or("credentials value missing")?,
            ),
            "remove" => {
                self.remove(name(&params)?)?;
                Ok(Value::Null)
            }
            _ => Err("credentials operation invalid".into()),
        }
    }

    fn prepare(&self) -> Result<(), String> {
        if let Some(base) = &self.trusted_base {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(base)
                .map_err(|_| "credentials data directory unavailable")?;
        }
        for dir in &self.dirs {
            ensure_private_dir(dir)?;
        }
        Ok(())
    }

    fn dir(&self) -> Result<File, String> {
        self.prepare()?;
        debug_assert_eq!(self.dirs.last(), Some(&self.root));
        let flags = OFlags::RDONLY
            | OFlags::DIRECTORY
            | OFlags::NOFOLLOW
            | OFlags::NONBLOCK
            | OFlags::CLOEXEC;
        // Anchor each provider-owned component to the previously checked descriptor.
        // Opening the full root path would follow a replaced provider-directory symlink.
        let mut parent = match &self.trusted_base {
            Some(base) => Some(File::from(
                open(base, flags, Mode::empty())
                    .map_err(|_| "credentials directory unavailable")?,
            )),
            None => None,
        };
        for path in &self.dirs {
            let fd = match &parent {
                Some(dir) => openat(
                    dir,
                    path.file_name().ok_or("credentials directory invalid")?,
                    flags,
                    Mode::empty(),
                ),
                None => open(path, flags, Mode::empty()),
            }
            .map_err(|_| "credentials directory unavailable")?;
            let file = File::from(fd);
            private_dir(
                &file
                    .metadata()
                    .map_err(|_| "credentials directory unavailable")?,
            )?;
            parent = Some(file);
        }
        parent.ok_or_else(|| "credentials directory unavailable".into())
    }

    fn list(&self) -> Result<Value, String> {
        let dir = self.dir()?;
        let entries = Dir::read_from(&dir).map_err(|_| "credentials list failed")?;
        let mut names = Vec::new();
        let mut count = 0;
        for entry in entries {
            let entry = entry.map_err(|_| "credentials list failed")?;
            let raw = entry.file_name().to_bytes();
            if raw == b"." || raw == b".." {
                continue;
            }
            count += 1;
            if count > MAX_ENTRIES {
                return Err("credentials entry limit exceeded".into());
            }
            let key = std::str::from_utf8(raw).map_err(|_| "credentials identifier invalid")?;
            // Crash leftovers are not records. Check them too; never follow or clean them up.
            let temporary = key.strip_prefix(".tmp-").is_some_and(|suffix| {
                suffix.len() == 32
                    && suffix
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            });
            if !temporary {
                valid_id(key)?;
            }
            let fd = openat(
                &dir,
                key,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| "credentials list failed")?;
            let file = File::from(fd);
            let metadata = file.metadata().map_err(|_| "credentials list failed")?;
            private_file(&metadata)?;
            if metadata.len() > MAX_BYTES as u64 {
                return Err("credentials data too large".into());
            }
            if !temporary {
                names.push(key.to_owned());
            }
        }
        names.sort_unstable();
        Ok(json!(names))
    }

    fn load(&self, key: &str) -> Result<Value, String> {
        valid_id(key)?;
        let dir = self.dir()?;
        let fd = match openat(
            &dir,
            key,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Value::Null),
            Err(_) => return Err("credentials read failed".into()),
        };
        let mut file = File::from(fd);
        private_file(&file.metadata().map_err(|_| "credentials read failed")?)?;
        let mut data = Vec::with_capacity(MAX_BYTES + 1);
        Read::by_ref(&mut file)
            .take((MAX_BYTES + 1) as u64)
            .read_to_end(&mut data)
            .map_err(|_| "credentials read failed")?;
        if data.len() > MAX_BYTES {
            return Err("credentials data too large".into());
        }
        serde_json::from_slice(&data).map_err(|_| "credentials data invalid".into())
    }

    fn save_new(&self, key: &str, value: &Value) -> Result<Value, String> {
        valid_id(key)?;
        let data = serde_json::to_vec(value).map_err(|_| "credentials value invalid")?;
        if data.len() > MAX_BYTES {
            return Err("credentials value too large".into());
        }
        let dir = self.dir()?;
        let mut random = [0u8; 16];
        OsRng
            .try_fill_bytes(&mut random)
            .map_err(|_| "credentials randomness unavailable")?;
        let temp = format!(".tmp-{:032x}", u128::from_le_bytes(random));
        let fd = openat(
            &dir,
            temp.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(|_| "credentials write failed")?;
        let mut file = File::from(fd);
        let write_result = (|| -> Result<(), String> {
            file.write_all(&data)
                .map_err(|_| "credentials write failed")?;
            file.sync_all().map_err(|_| "credentials write failed")?;
            linkat(&dir, temp.as_str(), &dir, key, AtFlags::empty()).map_err(|err| {
                if err.kind() == std::io::ErrorKind::AlreadyExists {
                    "credentials already exists"
                } else {
                    "credentials commit failed"
                }
            })?;
            // Once linked, a failed directory sync means the credential may exist but durability is unknown.
            fsync(&dir).map_err(|_| "credentials write outcome uncertain")?;
            Ok(())
        })();
        drop(file);
        let _ = unlinkat(&dir, temp.as_str(), AtFlags::empty());
        write_result?;
        Ok(json!(null))
    }

    fn remove(&self, key: &str) -> Result<(), String> {
        valid_id(key)?;
        let dir = self.dir()?;
        let fd = match openat(
            &dir,
            key,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err("credentials remove failed".into()),
        };
        let file = File::from(fd);
        private_file(&file.metadata().map_err(|_| "credentials remove failed")?)?;
        unlinkat(&dir, key, AtFlags::empty()).map_err(|_| "credentials remove failed")?;
        fsync(&dir).map_err(|_| "credentials remove outcome uncertain".to_string())
    }
}

fn name(params: &Value) -> Result<&str, String> {
    params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "credentials name invalid".into())
}

fn valid_id(value: &str) -> Result<(), String> {
    valid_identifier(value, MAX_ID)
}
fn valid_identifier(value: &str, limit: usize) -> Result<(), String> {
    if value.is_empty()
        || value.len() > limit
        || !value.is_ascii()
        || !value
            .bytes()
            .enumerate()
            .all(|(i, b)| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' && i != 0)
    {
        return Err("credentials identifier invalid".into());
    }
    Ok(())
}

fn ensure_private_dir(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => private_dir(&metadata),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(path)
                .map_err(|_| "credentials directory create failed")?;
            private_dir(
                &fs::symlink_metadata(path).map_err(|_| "credentials directory create failed")?,
            )
        }
        Err(_) => Err("credentials directory unavailable".into()),
    }
}

fn private_dir(metadata: &fs::Metadata) -> Result<(), String> {
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("credentials directory unsafe".into());
    }
    #[cfg(unix)]
    if !owned_private(metadata.uid(), metadata.permissions().mode()) {
        return Err("credentials directory not private".into());
    }
    Ok(())
}

fn private_file(metadata: &fs::Metadata) -> Result<(), String> {
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err("credentials file unsafe".into());
    }
    #[cfg(unix)]
    if !owned_private(metadata.uid(), metadata.permissions().mode()) {
        return Err("credentials file not private".into());
    }
    Ok(())
}

#[cfg(unix)]
fn owned_private(uid: u32, mode: u32) -> bool {
    uid == rustix::process::getuid().as_raw() && mode & 0o077 == 0
}

#[cfg(test)]
mod list_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "tela-credentials-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        root
    }
    fn store(app: &str) -> Store {
        let root = test_root();
        let app_root = root.join(app);
        Store {
            trusted_base: None,
            dirs: vec![app_root.clone()],
            root: app_root,
        }
    }
    #[test]
    fn source_hash_namespace_and_new_data_root() {
        let s = store("unused");
        let namespace = format!("remote-{}", "a".repeat(64));
        assert!(module_at(
            &namespace,
            s.root.join("missing-data-root").join("credentials")
        )
        .is_ok());
        assert!(
            valid_id(&namespace).is_err(),
            "credential names stay bounded separately"
        );
        let derived = Store {
            trusted_base: Some(s.root.join("data")),
            dirs: vec![
                s.root.join("data/provider"),
                s.root.join("data/provider").join(&namespace),
            ],
            root: s.root.join("data/provider").join(&namespace),
        };
        derived.save_new("profile", &json!({"test":true})).unwrap();
        assert_eq!(derived.load("profile").unwrap(), json!({"test":true}));
        fs::remove_dir_all(s.root.parent().unwrap()).unwrap();
    }
    #[test]
    fn save_load_and_no_overwrite() {
        let s = store("app");
        s.prepare().unwrap();
        assert_eq!(s.load("token").unwrap(), Value::Null);
        s.save_new("token", &json!({"opaque": true})).unwrap();
        assert_eq!(s.load("token").unwrap(), json!({"opaque": true}));
        assert_eq!(
            s.save_new("token", &json!(2)).unwrap_err(),
            "credentials already exists"
        );
        s.save_new("other", &json!(3)).unwrap();
        s.remove("token").unwrap();
        assert_eq!(s.load("token").unwrap(), Value::Null);
        assert_eq!(s.load("other").unwrap(), json!(3));
    }
    #[test]
    fn namespaces_and_invalid_input() {
        let root = test_root();
        let a_root = root.join("a");
        let b_root = root.join("b");
        let a = Store {
            trusted_base: None,
            dirs: vec![a_root.clone()],
            root: a_root,
        };
        let b = Store {
            trusted_base: None,
            dirs: vec![b_root.clone()],
            root: b_root,
        };
        a.save_new("key", &json!(1)).unwrap();
        assert_eq!(b.load("key").unwrap(), Value::Null);
        assert!(valid_id("../key").is_err());
        assert!(valid_id(" key").is_err());
        assert!(a.save_new("key2", &json!("x".repeat(MAX_BYTES))).is_err());
        fs::write(a.root.join("malformed"), b"not json").unwrap();
        fs::set_permissions(a.root.join("malformed"), fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(a.load("malformed").unwrap_err(), "credentials data invalid");
        fs::write(a.root.join("oversized"), vec![b'x'; MAX_BYTES + 1]).unwrap();
        fs::set_permissions(a.root.join("oversized"), fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            a.load("oversized").unwrap_err(),
            "credentials data too large"
        );
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlink_and_public_files() {
        let s = store("app");
        s.prepare().unwrap();
        std::os::unix::fs::symlink("/tmp", s.root.join("bad")).unwrap();
        assert!(s.load("bad").is_err());
        let unsafe_root = s.root.parent().unwrap().join("unsafe");
        std::os::unix::fs::symlink("/tmp", &unsafe_root).unwrap();
        let unsafe_store = Store {
            trusted_base: None,
            dirs: vec![unsafe_root.clone()],
            root: unsafe_root,
        };
        assert!(unsafe_store.prepare().is_err());
        assert!(std::process::Command::new("mkfifo")
            .arg(s.root.join("fifo"))
            .status()
            .unwrap()
            .success());
        assert!(s.load("fifo").is_err());
        assert!(s.remove("fifo").is_err());
        fs::write(s.root.join("public"), b"null").unwrap();
        fs::set_permissions(s.root.join("public"), fs::Permissions::from_mode(0o644)).unwrap();
        assert!(s.load("public").is_err());
    }
}
