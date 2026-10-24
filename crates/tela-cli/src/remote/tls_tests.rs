//! Actual loopback TLS, rather than only a fake downloader. Test CA never
//! enters the production loader or platform trust store.
use super::*;
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    thread,
};

struct Server {
    source: String,
    ca: Vec<u8>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Server {
    fn new(routes: HashMap<String, (u16, Option<String>, Vec<u8>)>) -> Self {
        let mut ca_params = CertificateParams::new(vec!["Tela test CA".into()]).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let ca_key = KeyPair::generate().unwrap();
        let ca = ca_params.self_signed(&ca_key).unwrap();
        let leaf_key = KeyPair::generate().unwrap();
        let leaf = CertificateParams::new(vec!["127.0.0.1".into()])
            .unwrap()
            .signed_by(&leaf_key, &ca, &ca_key)
            .unwrap();
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                vec![leaf.der().clone(), ca.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(leaf_key.serialize_der()).into(),
            )
            .unwrap();
        let config = Arc::new(config);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let source = format!("https://{}", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = thread::spawn(move || {
            while !stopping.load(Ordering::Acquire) {
                let (tcp, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(_) => break,
                };
                // macOS accept inherits the listener's nonblocking flag.
                tcp.set_nonblocking(false).unwrap();
                tcp.set_read_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                tcp.set_write_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                let mut tls = rustls::StreamOwned::new(
                    rustls::ServerConnection::new(config.clone()).unwrap(),
                    tcp,
                );
                let mut first = String::new();
                {
                    let mut reader = BufReader::new(&mut tls);
                    if reader.read_line(&mut first).is_err() {
                        continue;
                    }
                    for _ in 0..64 {
                        let mut line = String::new();
                        if reader.read_line(&mut line).is_err() || line == "\r\n" || line.is_empty()
                        {
                            break;
                        }
                    }
                }
                let path = first.split_whitespace().nth(1).unwrap_or("");
                let absent = (404, None, Vec::new());
                let (status, location, body) = routes.get(path).unwrap_or(&absent);
                let location = location
                    .as_ref()
                    .map(|v| format!("Location: {v}\r\n"))
                    .unwrap_or_default();
                let header = format!("HTTP/1.1 {status} Fixture\r\n{location}Content-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = tls.write_all(header.as_bytes());
                let _ = tls.write_all(body);
                let _ = tls.flush();
            }
        });
        Self {
            source,
            ca: ca.der().to_vec(),
            stop,
            thread: Some(thread),
        }
    }
    fn client(&self) -> HttpDownload {
        HttpDownload {
            client: Client::builder()
                .https_only(true)
                .no_proxy()
                .add_root_certificate(reqwest::Certificate::from_der(&self.ca).unwrap())
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            deadline: Duration::from_secs(3),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.thread.take() {
            worker.join().unwrap();
        }
    }
}
struct CacheRoot(PathBuf);
impl CacheRoot {
    fn new() -> Self {
        static ID: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "tela-tls-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self(root)
    }
}
impl Drop for CacheRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[derive(Default)]
struct Consent(usize);
impl Approval for Consent {
    fn approve(&mut self, _: &Grant) -> Result<()> {
        self.0 += 1;
        Ok(())
    }
}
fn routes(code: &[u8]) -> HashMap<String, (u16, Option<String>, Vec<u8>)> {
    let manifest = serde_json::to_vec(&serde_json::json!({"name":"TLS fixture","version":"1","entry":"bundle.js","sha256":hash(code),"permissions":[]})).unwrap();
    HashMap::from([
        (
            "/manifest.json".into(),
            (302, Some("/real-manifest.json".into()), vec![]),
        ),
        ("/real-manifest.json".into(), (200, None, manifest)),
        ("/bundle.js".into(), (200, None, code.to_vec())),
    ])
}
#[tokio::test]
async fn tls_manifest_redirect_bundle_and_cached_consent() {
    let code = b"var initialState = {};";
    let server = Server::new(routes(code));
    let cache = CacheRoot::new();
    let url = format!("{}/manifest.json", server.source);
    let mut consent = Consent::default();
    let app = prepare(&server.client(), &cache.0, &mut consent, &url)
        .await
        .unwrap();
    assert_eq!(app.bundle.as_bytes(), code);
    assert_eq!(app.namespace, format!("remote-{}", hash(url.as_bytes())));
    assert_eq!(consent.0, 1);
    prepare(&server.client(), &cache.0, &mut consent, &url)
        .await
        .unwrap();
    assert_eq!(consent.0, 1);
}
#[tokio::test]
async fn tls_rejects_untrusted_cert_cross_origin_and_wrong_hash() {
    let server = Server::new(routes(b"code"));
    assert!(HttpDownload::new()
        .unwrap()
        .get(
            &Url::parse(&format!("{}/manifest.json", server.source)).unwrap(),
            MANIFEST_LIMIT
        )
        .await
        .is_err());
    let mut bad_routes = routes(b"code");
    bad_routes.insert("/bundle.js".into(), (200, None, b"altered".to_vec()));
    bad_routes.insert(
        "/cross.json".into(),
        (
            302,
            Some("https://example.invalid/manifest.json".into()),
            vec![],
        ),
    );
    let server = Server::new(bad_routes);
    let cache = CacheRoot::new();
    let mut consent = Consent::default();
    assert!(prepare(
        &server.client(),
        &cache.0,
        &mut consent,
        &format!("{}/manifest.json", server.source)
    )
    .await
    .is_err());
    assert_eq!(consent.0, 0);
    assert!(server
        .client()
        .get(
            &Url::parse(&format!("{}/cross.json", server.source)).unwrap(),
            MANIFEST_LIMIT
        )
        .await
        .is_err());
}

#[tokio::test]
#[ignore = "requires controlling PTY harness, synthetic app only"]
async fn remote_approval_and_engine_on_pty() {
    assert_eq!(std::env::var("TELA_REMOTE_PTY_TEST").as_deref(), Ok("1"));
    let code=b"var initialState={done:false}; var keybindings={normal:{q:'quit',Enter:'start'}}; function reduce(s,a){if(a.type==='start')Promise.resolve().then(()=>Tela.dispatch({type:'done'}));return a.type==='done'?{done:true}:s;} function view(s){return h('text',{},s.done?'111111111111111111111111':'URL_FIXTURE_READY');}";
    let server = Server::new(routes(code));
    let cache = CacheRoot::new();
    let app = prepare(
        &server.client(),
        &cache.0,
        &mut TerminalApproval,
        &format!("{}/manifest.json", server.source),
    )
    .await
    .unwrap();
    let engine = tela_engine::Engine::new_with_manifest(&app.namespace, &app.permissions)
        .await
        .unwrap();
    engine.load_bundle(&app.bundle).await.unwrap();
    engine.run().await.unwrap();
}
