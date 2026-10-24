//! Private CA injection is Rust-host-only. JS never controls TLS verification.
use super::*;
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use tokio_rustls::{rustls, TlsAcceptor};

fn certificates() -> (TlsAcceptor, WsConfig) {
    let mut ca_params = CertificateParams::new(vec!["TLS fixture CA".into()]).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let key = KeyPair::generate().unwrap();
    let leaf = CertificateParams::new(vec!["127.0.0.1".into()])
        .unwrap()
        .signed_by(&key, &ca, &ca_key)
        .unwrap();
    let server = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![leaf.der().clone(), ca.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
        )
        .unwrap();
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca.der().clone()).unwrap();
    let client = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    (
        TlsAcceptor::from(Arc::new(server)),
        WsConfig {
            connector: Some(Connector::Rustls(Arc::new(client))),
        },
    )
}

#[tokio::test]
async fn trusted_local_tls_negotiates_protocol_and_binary() {
    let (tls, config) = certificates();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let stream = tls.accept(tcp).await.unwrap();
        let mut ws = accept_hdr_async(stream, |request: &Request, mut response: ServerResponse| {
            assert_eq!(request.headers()[header::ORIGIN], "https://example.invalid");
            assert_eq!(
                request.headers()[header::SEC_WEBSOCKET_PROTOCOL],
                "fixture.v1"
            );
            response.headers_mut().insert(
                header::SEC_WEBSOCKET_PROTOCOL,
                HeaderValue::from_static("fixture.v1"),
            );
            Ok(response)
        })
        .await
        .unwrap();
        let frame = ws.next().await.unwrap().unwrap();
        assert!(matches!(&frame,Message::Binary(bytes) if bytes.as_ref()==[1,2,3]));
        ws.send(frame).await.unwrap();
        let _ = ws.next().await;
    });
    let mut h = Harness::with_config(config);
    h.eval(&format!(r#"
        globalThis.done=false;globalThis.failed=false;
        const ws=new WebSocket('wss://{address}/fixture',['fixture.v1'],{{origin:'https://example.invalid'}});
        ws.onopen=()=>ws.send(new Uint8Array([9,1,2,3,9]).subarray(1,4));
        ws.onerror=()=>{{failed=true;done=true;}};
        ws.onmessage=e=>{{
            const bytes=new Uint8Array(e.data);
            failed=ws.protocol!=='fixture.v1'||bytes.length!==3||bytes[0]!==1||bytes[2]!==3;
            done=true;ws.close();
        }};
    "#));
    h.until("done").await;
    assert!(!h.boolean("failed"));
    shutdown_ws(&h.registry);
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn default_tls_rejects_private_untrusted_ca() {
    let (tls, _) = certificates();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        assert!(tls.accept(tcp).await.is_err());
    });
    let mut h = Harness::new();
    h.eval(&format!(
        r#"
        globalThis.closed=false;globalThis.failed=false;globalThis.opened=false;
        const ws=new WebSocket('wss://{address}/fixture');
        ws.onopen=()=>{{opened=true;}};
        ws.onerror=()=>{{failed=true;}};
        ws.onclose=()=>{{closed=true;}};
    "#
    ));
    h.until("closed").await;
    assert!(h.boolean("failed && !opened"));
    shutdown_ws(&h.registry);
    tokio::time::timeout(Duration::from_secs(2), server)
        .await
        .unwrap()
        .unwrap();
}
