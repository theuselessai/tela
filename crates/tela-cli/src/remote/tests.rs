use super::*;
use clap::Parser;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::{symlink, DirBuilderExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const SOURCE: &str = "https://example.invalid/apps/manifest.json";
const CODE: &str = "var initialState = { downloaded: true };";

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().canonicalize().unwrap();
        let path = base.join(format!(
            "tela-loader-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    fn cache(&self) -> PathBuf {
        self.0.join("cache")
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct Downloads {
    bodies: HashMap<String, Vec<u8>>,
    requests: RefCell<Vec<String>>,
}
impl Download for Downloads {
    async fn get(&self, url: &Url, _limit: usize) -> Result<Vec<u8>> {
        self.requests.borrow_mut().push(url.to_string());
        self.bodies
            .get(url.as_str())
            .cloned()
            .context("unexpected fixture URL")
    }
}
impl Downloads {
    fn app(&mut self, source: &str, code: &[u8], permissions: &[&str]) {
        let manifest = serde_json::json!({
            "name": "not-an-identity", "version": "1", "entry": "bundle.js",
            "sha256": hash(code), "permissions": permissions,
        });
        self.bodies
            .insert(source.to_owned(), serde_json::to_vec(&manifest).unwrap());
        self.bodies.insert(
            Url::parse(source)
                .unwrap()
                .join("bundle.js")
                .unwrap()
                .to_string(),
            code.to_vec(),
        );
    }
    fn alter(&mut self, source: &str, field: &str, value: serde_json::Value) {
        let mut manifest: serde_json::Value = serde_json::from_slice(&self.bodies[source]).unwrap();
        manifest[field] = value;
        self.bodies
            .insert(source.to_owned(), serde_json::to_vec(&manifest).unwrap());
    }
}

#[derive(Default)]
struct Consent {
    count: usize,
    deny: bool,
}
impl Approval for Consent {
    fn approve(&mut self, _grant: &Grant) -> Result<()> {
        self.count += 1;
        ensure!(!self.deny, "fixture denied approval");
        Ok(())
    }
}

#[test]
fn cli_requires_exactly_one_run_source() {
    assert!(crate::Cli::try_parse_from(["tela", "run"]).is_err());
    assert!(crate::Cli::try_parse_from(["tela", "run", ".", "--url", SOURCE]).is_err());
    assert!(crate::Cli::try_parse_from(["tela", "run", "--url", SOURCE]).is_ok());
    assert!(crate::Cli::try_parse_from(["tela", "run", "."]).is_ok());
    assert!(crate::Cli::try_parse_from(["tela", "dev", "."]).is_ok());
    assert!(crate::Cli::try_parse_from(["tela", "init", "demo"]).is_ok());
}

#[test]
fn urls_and_entries_cannot_escape_policy() {
    for bad in [
        "http://example.invalid/a",
        "file:///a",
        "https://user@example.invalid/a",
        "https://@example.invalid/a",
        "https://example.invalid/a#",
        "https://example.invalid/a#x",
        "https://example.invalid/\nx",
        "https://example.invalid/a b",
        "https:example.invalid",
        "https:///example.invalid/a",
        "https:\\\\@example.invalid/a",
    ] {
        assert!(source_url(bad).is_err(), "{bad}");
    }
    let source = source_url(SOURCE).unwrap();
    for bad in [
        "",
        "../bundle.js",
        "./bundle.js",
        "/bundle.js",
        "//evil.invalid/a.js",
        "https://evil.invalid/a.js",
        "a//b.js",
        "a/../b.js",
        "a\\b.js",
        "%2e%2e/a.js",
        "a%2fb.js",
        "a.js?q=1",
        "a.js#fragment",
        "native.so",
        "a.js\n",
        "a:b.js",
    ] {
        assert!(bundle_url(&source, bad).is_err(), "{bad}");
    }
    assert_eq!(
        bundle_url(&source, "dist/app.min.js").unwrap().as_str(),
        "https://example.invalid/apps/dist/app.min.js"
    );
    for target in [
        "https://evil.invalid/a.js",
        "http://example.invalid/a.js",
        "https://example.invalid:444/a.js",
        "//evil.invalid/a.js",
        "https://user@example.invalid/a.js",
        "https://@example.invalid/a.js",
        "a.js#x",
    ] {
        assert!(
            redirect_target(&source, &source, target).is_err(),
            "{target}"
        );
    }
    assert!(redirect_target(&source, &source, "/other.js").is_ok());
    assert!(redirect_target(&source, &source, "https://EXAMPLE.invalid:443/other.js").is_ok());
}

#[test]
fn approval_is_terminal_explicit_and_eof_closed() {
    let grant = Grant {
        source: "https://example.invalid/\u{1b}[31m".into(),
        sha256: hash(CODE.as_bytes()),
        permissions: vec![Permission::Storage],
    };
    for answer in [
        "", "y", "yes", "n\n", "YES\n", " yes\n", "yes \n", "maybe\n",
    ] {
        assert!(
            ask(&mut io::Cursor::new(answer), &mut Vec::new(), true, &grant).is_err(),
            "{answer:?}"
        );
    }
    assert!(ask(
        &mut io::Cursor::new("yes\n"),
        &mut Vec::new(),
        false,
        &grant
    )
    .is_err());
    for answer in ["y\n", "yes\n", "yes\r\n"] {
        let mut output = Vec::new();
        ask(&mut io::Cursor::new(answer), &mut output, true, &grant).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(!output.contains('\u{1b}'));
        assert!(output.contains(&grant.sha256));
        assert!(output.contains("storage"));
    }
}

#[tokio::test]
async fn approved_code_runs_in_stock_engine_without_native_modules() {
    let temp = Temp::new();
    let mut downloads = Downloads::default();
    downloads.app(SOURCE, CODE.as_bytes(), &[]);
    let app = prepare(&downloads, &temp.cache(), &mut Consent::default(), SOURCE)
        .await
        .unwrap();
    let engine = tela_engine::Engine::new_with_manifest(&app.namespace, &app.permissions)
        .await
        .unwrap();
    engine.load_bundle(&app.bundle).await.unwrap();
    assert_eq!(
        engine.get_initial_state_json().await.unwrap(),
        serde_json::json!({ "downloaded": true })
    );
}

#[tokio::test]
async fn grants_bind_source_code_and_permissions_not_name_or_version() {
    let temp = Temp::new();
    let mut downloads = Downloads::default();
    let mut consent = Consent::default();
    downloads.app(SOURCE, CODE.as_bytes(), &["storage", "network"]);
    let first = prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap();
    assert_eq!(consent.count, 1);
    assert_eq!(
        first.namespace,
        format!("remote-{}", hash(SOURCE.as_bytes()))
    );
    assert_eq!(first.permissions, ["network", "storage"]);
    // Same digest: the manifest is fetched again but bundle bytes come from cache.
    downloads.requests.borrow_mut().clear();
    downloads.alter(SOURCE, "name", serde_json::json!("../../other-app"));
    downloads.alter(SOURCE, "version", serde_json::json!("2"));
    downloads.alter(
        SOURCE,
        "permissions",
        serde_json::json!(["network", "storage", "network"]),
    );
    let second = prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap();
    assert_eq!(second.namespace, first.namespace);
    assert_eq!(consent.count, 1);
    assert_eq!(*downloads.requests.borrow(), [SOURCE]);
    // A permission change without a version bump must prompt.
    downloads.alter(
        SOURCE,
        "permissions",
        serde_json::json!(["storage", "credentials"]),
    );
    prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap();
    assert_eq!(consent.count, 2);
    // So must removing permissions, not just escalating them.
    downloads.alter(SOURCE, "permissions", serde_json::json!(["storage"]));
    prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap();
    assert_eq!(consent.count, 3);
    // Code changes also prompt, even with the same manifest version/name.
    downloads.app(SOURCE, b"var initialState = 2;", &["storage"]);
    let changed = prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap();
    assert_eq!(consent.count, 4);
    assert_eq!(changed.namespace, first.namespace);
    let other = "https://example.invalid/other/manifest.json";
    downloads.app(other, b"var initialState = 2;", &["storage"]);
    let other_app = prepare(&downloads, &temp.cache(), &mut consent, other)
        .await
        .unwrap();
    assert_ne!(other_app.namespace, first.namespace);
    assert_eq!(consent.count, 5);
    // URL canonicalization gives the same source identity.
    let canonical = prepare(
        &downloads,
        &temp.cache(),
        &mut consent,
        "https://EXAMPLE.invalid:443/apps/manifest.json",
    )
    .await
    .unwrap();
    assert_eq!(canonical.namespace, first.namespace);
    assert_eq!(consent.count, 5);
}

#[tokio::test]
async fn denial_saves_nothing_and_changed_code_cannot_reuse_old_grant() {
    let temp = Temp::new();
    let mut downloads = Downloads::default();
    let mut consent = Consent {
        deny: true,
        count: 0,
    };
    downloads.app(SOURCE, CODE.as_bytes(), &[]);
    assert!(prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .is_err());
    assert_eq!(fs::read_dir(temp.cache()).unwrap().count(), 0);
    consent.deny = false;
    prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap();
    let grant_path = temp
        .cache()
        .join(format!("grant-{}", hash(SOURCE.as_bytes())));
    let before = fs::read(&grant_path).unwrap();
    downloads.app(SOURCE, b"throw new Error('changed');", &[]);
    consent.deny = true;
    assert!(prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .is_err());
    assert_eq!(fs::read(&grant_path).unwrap(), before);
    consent.deny = false;
    prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap();
    assert_eq!(consent.count, 4);
}

#[tokio::test]
async fn wrong_hash_and_invalid_utf8_never_reach_approval() {
    let temp = Temp::new();
    let mut downloads = Downloads::default();
    let mut consent = Consent::default();
    downloads.app(SOURCE, CODE.as_bytes(), &[]);
    downloads.alter(SOURCE, "sha256", serde_json::json!("0".repeat(64)));
    assert!(prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap_err_string()
        .contains("SHA-256 mismatch"));
    downloads.app(SOURCE, &[0xff], &[]);
    assert!(prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap_err_string()
        .contains("UTF-8"));
    downloads.bodies.insert(SOURCE.into(), vec![0xff]);
    assert!(prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .is_err());
    assert_eq!(consent.count, 0);
}

// Avoid requiring Debug on the successful App solely for error assertions.
trait ErrorString {
    fn unwrap_err_string(self) -> String;
}
impl ErrorString for Result<App> {
    fn unwrap_err_string(self) -> String {
        match self {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected error"),
        }
    }
}

#[tokio::test]
async fn strict_manifest_rejects_ambient_permissions_and_bad_fields() {
    let temp = Temp::new();
    let mut downloads = Downloads::default();
    let mut consent = Consent::default();
    for permissions in [
        serde_json::json!(["env"]),
        serde_json::json!(["filesystem"]),
        serde_json::json!(["filesystem:read"]),
        serde_json::json!(["unknown"]),
        serde_json::json!([1]),
        serde_json::json!("network"),
        serde_json::Value::Null,
    ] {
        downloads.app(SOURCE, CODE.as_bytes(), &[]);
        downloads.alter(SOURCE, "permissions", permissions);
        assert!(prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
            .await
            .is_err());
    }
    for (field, value) in [
        ("sha256", serde_json::json!("")),
        ("sha256", serde_json::json!("z".repeat(64))),
        ("sha256", serde_json::Value::Null),
        ("entry", serde_json::json!("../escape.js")),
        ("entry", serde_json::json!("https://evil.invalid/code.js")),
        ("commands", serde_json::json!(["npm install"])),
    ] {
        downloads.app(SOURCE, CODE.as_bytes(), &[]);
        downloads.alter(SOURCE, field, value);
        assert!(prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
            .await
            .is_err());
    }
    assert_eq!(consent.count, 0);
    downloads.app(
        SOURCE,
        CODE.as_bytes(),
        &["network", "storage", "crypto", "credentials", "clipboard"],
    );
    assert!(prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .is_ok());
}

#[tokio::test]
async fn limits_apply_even_with_injected_downloader() {
    let temp = Temp::new();
    let mut downloads = Downloads::default();
    let mut consent = Consent::default();
    downloads
        .bodies
        .insert(SOURCE.into(), vec![b' '; MANIFEST_LIMIT + 1]);
    assert!(prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .is_err());
    downloads.app(SOURCE, &vec![b' '; BUNDLE_LIMIT + 1], &[]);
    assert!(prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .is_err());
    assert_eq!(consent.count, 0);
    let mut streamed = vec![0; 4];
    assert!(append_chunk(&mut streamed, &[1; 4], 8).is_ok());
    assert!(append_chunk(&mut streamed, &[2], 8).is_err());
    assert_eq!(streamed.len(), 8);
}

#[tokio::test]
async fn cache_tampering_is_detected_even_with_existing_grant() {
    let temp = Temp::new();
    let mut downloads = Downloads::default();
    let mut consent = Consent::default();
    downloads.app(SOURCE, CODE.as_bytes(), &[]);
    prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap();
    let bundle = temp
        .cache()
        .join(format!("bundle-{}", hash(CODE.as_bytes())));
    fs::write(&bundle, b"malicious changed cache").unwrap();
    assert!(prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap_err_string()
        .contains("SHA-256 mismatch"));
    assert_eq!(consent.count, 1);
    // Missing data is downloaded and hash-checked again; existing consent suffices.
    fs::remove_file(&bundle).unwrap();
    prepare(&downloads, &temp.cache(), &mut consent, SOURCE)
        .await
        .unwrap();
    assert_eq!(consent.count, 1);
    assert_eq!(fs::read(&bundle).unwrap(), CODE.as_bytes());
}

#[test]
fn cache_rejects_symlinks_hardlinks_public_modes_and_unsafe_keys() {
    let temp = Temp::new();
    let root = temp.cache();
    let cache = cache::Cache::open(&root).unwrap();
    assert_eq!(
        fs::metadata(&root).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let key = format!("bundle-{}", hash(CODE.as_bytes()));
    cache.write(&key, CODE.as_bytes()).unwrap();
    assert_eq!(
        fs::metadata(root.join(&key)).unwrap().permissions().mode() & 0o777,
        0o600
    );
    for bad in [
        "../escape",
        "/absolute",
        "grant-../../outside",
        "bundle-x",
        ".tmp-1-2",
    ] {
        assert!(cache.read(bad, 100).is_err());
        assert!(cache.write(bad, b"no").is_err());
    }
    fs::hard_link(root.join(&key), temp.0.join("hardlink")).unwrap();
    assert!(cache.read(&key, 100).is_err());
    assert!(cache.write(&key, b"changed").is_err());
    fs::remove_file(root.join(&key)).unwrap();
    symlink(temp.0.join("hardlink"), root.join(&key)).unwrap();
    assert!(cache.read(&key, 100).is_err());
    assert!(cache.write(&key, b"changed").is_err());
    assert_eq!(fs::read(temp.0.join("hardlink")).unwrap(), CODE.as_bytes());
    let link = temp.0.join("link");
    symlink(&root, &link).unwrap();
    assert!(cache::Cache::open(&link).is_err());
    assert!(cache::Cache::open(&link.join("nested")).is_err());
    assert!(cache::Cache::open(&root.join("../escape")).is_err());
    assert!(cache::Cache::open(Path::new("relative")).is_err());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(cache::Cache::open(&root).is_err());
}

#[tokio::test]
async fn untrusted_manifest_errors_cannot_inject_terminal_controls() {
    let temp = Temp::new();
    let mut downloads = Downloads::default();
    downloads.app(SOURCE, CODE.as_bytes(), &["\u{1b}[31m"]);
    let error = prepare(&downloads, &temp.cache(), &mut Consent::default(), SOURCE)
        .await
        .unwrap_err_string();
    assert!(!error.contains('\u{1b}'));
    assert!(error.contains("invalid remote manifest"));
}

#[tokio::test]
async fn https_transport_rejects_plaintext_and_stalled_handshakes() {
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\ncode")
            .await
            .unwrap();
    });
    let client = HttpDownload {
        client: Client::builder()
            .https_only(true)
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap(),
        deadline: Duration::from_secs(2),
    };
    let url = Url::parse(&format!("https://{address}/manifest.json")).unwrap();
    assert!(client.get(&url, MANIFEST_LIMIT).await.is_err());
    server.await.unwrap();
    assert!(client
        .get(
            &Url::parse("http://127.0.0.1/manifest.json").unwrap(),
            MANIFEST_LIMIT
        )
        .await
        .is_err());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (accepted, receive) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let _ = accepted.send(stream);
    });
    let client = HttpDownload {
        deadline: Duration::from_millis(50),
        ..client
    };
    let url = Url::parse(&format!("https://{address}/manifest.json")).unwrap();
    let (result, stream) = tokio::join!(client.get(&url, MANIFEST_LIMIT), receive);
    let _stream = stream.unwrap();
    assert!(result.unwrap_err().to_string().contains("timed out"));
    server.await.unwrap();
}

#[test]
fn cache_atomic_replacement_and_file_modes() {
    let temp = Temp::new();
    let cache = cache::Cache::open(&temp.cache()).unwrap();
    let key = format!("grant-{}", hash(SOURCE.as_bytes()));
    cache.write(&key, b"before").unwrap();
    cache.write(&key, b"after").unwrap();
    assert_eq!(cache.read(&key, 10).unwrap().unwrap(), b"after");
    assert_eq!(fs::read_dir(temp.cache()).unwrap().count(), 1);
    assert!(cache.read(&key, 4).is_err());
    fs::set_permissions(temp.cache().join(&key), fs::Permissions::from_mode(0o644)).unwrap();
    assert!(cache.read(&key, 10).is_err());
    assert!(cache.write(&key, b"new").is_err());
}
