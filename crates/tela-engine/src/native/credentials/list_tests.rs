//! Isolated filesystem fixtures only; never use credentials_module's OS data root.
use super::*;
use std::os::unix::fs::{symlink, DirBuilderExt, PermissionsExt};

struct Fixture(Store);
impl Fixture {
    fn new() -> Self {
        let mut random = [0; 16];
        OsRng.fill_bytes(&mut random);
        let root = std::env::temp_dir().join(format!(
            "tela-list-test-{:032x}",
            u128::from_le_bytes(random)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self(Store {
            trusted_base: None,
            dirs: vec![root.clone()],
            root,
        })
    }
    fn file(&self, name: &str, data: &[u8]) {
        let path = self.0.root.join(name);
        fs::write(&path, data).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0.root).unwrap();
    }
}

#[test]
fn empty_one_many_legacy_names_only_no_overwrite() {
    let f = Fixture::new();
    assert_eq!(f.0.dispatch("list", Value::Null).unwrap(), json!([]));
    f.0.save_new("profile", &json!({"secret": "synthetic"}))
        .unwrap();
    assert_eq!(f.0.list().unwrap(), json!(["profile"]));
    for name in ["host_b", "host_a"] {
        f.0.save_new(name, &json!({"secret": "synthetic"})).unwrap();
    }
    assert_eq!(f.0.list().unwrap(), json!(["host_a", "host_b", "profile"]));
    assert!(f.0.save_new("profile", &json!("replacement")).is_err());
    assert_eq!(f.0.load("profile").unwrap(), json!({"secret": "synthetic"}));
    // Enumeration intentionally does not parse opaque content. Loading does.
    f.file("corrupt", b"not JSON: synthetic secret");
    assert!(f
        .0
        .list()
        .unwrap()
        .as_array()
        .unwrap()
        .contains(&json!("corrupt")));
    assert_eq!(f.0.load("corrupt").unwrap_err(), "credentials data invalid");
}

#[test]
fn bounded_names_sizes_and_number_including_temporary_files() {
    let f = Fixture::new();
    let long = "a".repeat(MAX_ID);
    f.file(&long, b"null");
    assert_eq!(f.0.list().unwrap(), json!([long]));
    fs::remove_file(f.0.root.join(&long)).unwrap();
    for name in [
        ".hidden".to_owned(),
        ".tmp-not-valid".to_owned(),
        "a".repeat(MAX_ID + 1),
        "bad name".to_owned(),
        "é".to_owned(),
    ] {
        f.file(&name, b"null");
        assert!(f.0.list().is_err());
        fs::remove_file(f.0.root.join(name)).unwrap();
    }
    f.file("large", &vec![b' '; MAX_BYTES + 1]);
    assert_eq!(f.0.list().unwrap_err(), "credentials data too large");
    fs::remove_file(f.0.root.join("large")).unwrap();
    f.file("maximum", &vec![b' '; MAX_BYTES]);
    assert_eq!(f.0.list().unwrap(), json!(["maximum"]));
    fs::remove_file(f.0.root.join("maximum")).unwrap();
    let invalid_utf8 = std::ffi::OsString::from_vec(vec![0xff]);
    use std::os::unix::ffi::OsStringExt;
    let invalid_path = f.0.root.join(invalid_utf8);
    match fs::write(&invalid_path, b"null") {
        Ok(()) => {
            assert!(f.0.list().is_err());
            fs::remove_file(invalid_path).unwrap();
        }
        Err(error) if error.raw_os_error() == Some(rustix::io::Errno::ILSEQ.raw_os_error()) => {
            eprintln!("filesystem itself rejects non-UTF-8 names");
        }
        Err(error) => panic!("invalid-name fixture create failed: {error}"),
    }
    for n in 0..MAX_ENTRIES {
        f.file(&format!(".tmp-{n:032x}"), b"null");
    }
    assert_eq!(f.0.list().unwrap(), json!([]));
    f.file("overflow", b"null");
    assert_eq!(f.0.list().unwrap_err(), "credentials entry limit exceeded");
    fs::remove_file(f.0.root.join("overflow")).unwrap();
    for n in 0..MAX_ENTRIES {
        fs::rename(
            f.0.root.join(format!(".tmp-{n:032x}")),
            f.0.root.join(format!("item_{n}")),
        )
        .unwrap();
    }
    assert_eq!(f.0.list().unwrap().as_array().unwrap().len(), MAX_ENTRIES);
    f.file("overflow", b"null");
    assert!(f.0.list().is_err());
}

#[test]
fn rejects_symlink_fifo_directory_and_public_file_without_reading() {
    let f = Fixture::new();
    let bad = f.0.root.join("bad");
    symlink("missing", &bad).unwrap();
    assert!(f.0.list().is_err());
    fs::remove_file(&bad).unwrap();
    assert!(std::process::Command::new("mkfifo")
        .arg(&bad)
        .status()
        .unwrap()
        .success());
    fs::set_permissions(&bad, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(f.0.list().is_err()); // O_NONBLOCK: no writer needed and no hang.
    fs::remove_file(&bad).unwrap();
    fs::DirBuilder::new().mode(0o700).create(&bad).unwrap();
    assert!(f.0.list().is_err());
    fs::remove_dir(&bad).unwrap();
    f.file("bad", b"null");
    fs::set_permissions(&bad, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(f.0.list().is_err());
    fs::remove_file(&bad).unwrap();
    f.file(".tmp-00000000000000000000000000000000", b"null");
    fs::set_permissions(
        f.0.root.join(".tmp-00000000000000000000000000000000"),
        fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(f.0.list().is_err());
}

#[test]
fn rejects_unsafe_directories_and_isolates_namespaces() {
    let f = Fixture::new();
    let provider = f.0.root.join("provider");
    let app = provider.join("app");
    let s = Store {
        trusted_base: Some(f.0.root.clone()),
        dirs: vec![provider.clone(), app.clone()],
        root: app,
    };
    s.save_new("profile", &json!(1)).unwrap();
    assert_eq!(f.0.load("profile").unwrap(), Value::Null);
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(s.list().is_err());
    fs::set_permissions(&provider, fs::Permissions::from_mode(0o700)).unwrap();
    let renamed = f.0.root.join("renamed");
    fs::rename(&provider, &renamed).unwrap();
    symlink(&renamed, &provider).unwrap();
    assert!(s.list().is_err());
    fs::remove_file(&provider).unwrap();
}

#[test]
fn rejects_foreign_owner_and_checks_real_fixture_when_chown_is_permitted() {
    let f = Fixture::new();
    f.file("foreign", b"null");
    let foreign = f.0.root.join("foreign");
    let uid = rustix::process::getuid().as_raw();
    let other = rustix::process::Uid::from_raw(if uid == 0 { 1 } else { 0 });
    // The same predicate is used on fstat data for both files and directories.
    assert!(owned_private(uid, 0o600));
    assert!(owned_private(uid, 0o700));
    assert!(!owned_private(other.as_raw(), 0o600));
    assert!(!owned_private(other.as_raw(), 0o700));
    assert!(!owned_private(uid, 0o640));
    assert!(!owned_private(uid, 0o701));
    if rustix::fs::chown(&foreign, Some(other), None).is_ok() {
        assert_eq!(f.0.list().unwrap_err(), "credentials file not private");
    } else {
        eprintln!("foreign-owner filesystem fixture unavailable: chown requires privilege");
    }
}
