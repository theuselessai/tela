use super::*;
#[path = "websocket_tls_tests.rs"]
mod tls;
use rquickjs::{Context, Runtime};
use tokio::net::TcpListener;
use tokio_tungstenite::{
    accept_async, accept_hdr_async, tungstenite::handshake::server::Response as ServerResponse,
};

struct Harness {
    context: Context,
    _runtime: Runtime,
    registry: WsSenders,
    actions: mpsc::UnboundedReceiver<serde_json::Value>,
}

impl Harness {
    fn new() -> Self {
        Self::with_config(WsConfig::default())
    }

    fn with_config(config: WsConfig) -> Self {
        let runtime = Runtime::new().unwrap();
        let context = Context::full(&runtime).unwrap();
        let registry = Arc::new(Mutex::new(HashMap::new()));
        let (tx, actions) = mpsc::unbounded_channel();
        context.with(|ctx| {
            register_websocket_with_config(
                &ctx,
                tx,
                registry.clone(),
                tokio::runtime::Handle::current(),
                config,
            )
            .unwrap()
        });
        Self {
            context,
            _runtime: runtime,
            registry,
            actions,
        }
    }

    fn eval(&self, source: &str) {
        self.context.with(|ctx| {
            if ctx.eval::<(), _>(source).is_err() {
                panic!("JS test failed: {:?}", ctx.catch());
            }
        });
    }

    fn boolean(&self, source: &str) -> bool {
        self.context.with(|ctx| ctx.eval(source).unwrap())
    }

    async fn dispatch(&mut self) {
        let action = tokio::time::timeout(Duration::from_secs(3), self.actions.recv())
            .await
            .expect("WebSocket action timeout")
            .unwrap();
        assert_eq!(action["event"], "drain");
        assert_eq!(action["data"], "");
        self.context.with(|ctx| {
            ctx.globals()
                .get::<_, Function>("__tela_ws_event__")
                .unwrap()
                .call::<_, ()>((action["id"].as_u64().unwrap(), "drain", ""))
                .unwrap();
        });
    }

    async fn until(&mut self, predicate: &str) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while !self.boolean(predicate) {
                self.dispatch().await;
            }
        })
        .await
        .expect("JS predicate timeout");
    }
}

async fn listener() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    (listener, url)
}

#[tokio::test]
async fn quickjs_binary_text_protocol_origin_roundtrip() {
    let (listener, url) = listener().await;
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket =
            accept_hdr_async(stream, |request: &Request, mut response: ServerResponse| {
                assert_eq!(request.headers()[header::ORIGIN], "https://client.example");
                assert_eq!(
                    request.headers()[header::SEC_WEBSOCKET_PROTOCOL],
                    "fixture.v2, other"
                );
                response.headers_mut().insert(
                    header::SEC_WEBSOCKET_PROTOCOL,
                    HeaderValue::from_static("fixture.v2"),
                );
                Ok(response)
            })
            .await
            .unwrap();
        for i in 0..4 {
            let message = socket.next().await.unwrap().unwrap();
            match (&message, i) {
                (Message::Binary(bytes), 0) => assert_eq!(bytes.as_ref(), &[1, 2, 255]),
                (Message::Binary(bytes), 1) => assert_eq!(bytes.as_ref(), &[8, 0, 9]),
                (Message::Text(text), 2) => assert_eq!(text.as_str(), "hello \u{1f30d}"),
                (Message::Binary(bytes), 3) => {
                    assert_eq!(bytes.len(), 262168);
                    assert_eq!(bytes[262167], 123);
                }
                _ => panic!("unexpected echo message"),
            }
            socket.send(message).await.unwrap();
        }
        // Client close must terminate even while the peer sends no close frame.
        let _ = tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap();
    });
    let mut h = Harness::new();
    h.eval(&format!(r#"
        globalThis.ws = new WebSocket({url:?}, ['fixture.v2', 'other'], {{origin:'https://client.example'}});
        globalThis.received = [];
        globalThis.closed = false;
        ws.binaryType = 'arraybuffer';
        ws.onopen = function() {{
            if (this !== ws || ws.protocol !== 'fixture.v2') throw Error('open/protocol');
            ws.send(new Uint8Array([99,1,2,255,99]).subarray(1,4));
            ws.send(new Uint8Array([8,0,9]).buffer);
            ws.send('hello 🌍');
            let large = new Uint8Array(262168); large[262167] = 123; ws.send(large);
        }};
        ws.onmessage = e => {{
            received.push(typeof e.data === 'string' ? e.data : Array.from(new Uint8Array(e.data)));
            if (typeof e.data !== 'string' && !(e.data instanceof ArrayBuffer)) throw Error('not ArrayBuffer');
            if (received.length === 4) ws.close();
        }};
        ws.onclose = () => {{closed = true;}};
    "#));
    h.until("closed").await;
    assert!(h.boolean("JSON.stringify(received[0]) === '[1,2,255]' && JSON.stringify(received[1]) === '[8,0,9]' && received[2] === 'hello 🌍' && received[3].length === 262168 && received[3][262167] === 123 && ws.readyState === WebSocket.CLOSED"));
    assert!(h.registry.lock().unwrap().is_empty());
    server.await.unwrap();
}

#[tokio::test]
async fn quickjs_flush_writes_before_close_and_close_rejects_pending() {
    let (first_listener, url) = listener().await;
    let server = tokio::spawn(async move {
        let (peer, _) = first_listener.accept().await.unwrap();
        let mut socket = accept_async(peer).await.unwrap();
        assert!(
            matches!(socket.next().await.unwrap().unwrap(), Message::Binary(bytes) if bytes.as_ref() == [4, 5, 6])
        );
        tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap();
    });
    let mut h = Harness::new();
    h.eval(&format!(
        r#"
        globalThis.done = globalThis.failed = false;
        globalThis.ws = new WebSocket({url:?});
        ws.onopen = () => {{
            ws.send(new Uint8Array([4,5,6]));
            ws.flush().then(() => {{ done = true; ws.close(); }}, () => {{ failed = true; }});
        }};
    "#
    ));
    tokio::time::timeout(Duration::from_secs(5), async {
        while !h.boolean("done && ws.readyState === WebSocket.CLOSED") {
            h.dispatch().await;
            while h._runtime.execute_pending_job().unwrap() {}
        }
    })
    .await
    .unwrap();
    assert!(!h.boolean("failed"));
    server.await.unwrap();

    let (listener, url) = listener().await;
    let server = tokio::spawn(async move {
        let (peer, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(peer).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .unwrap();
    });
    let mut h = Harness::new();
    h.eval(&format!(
        r#"
        globalThis.resolved = 0; globalThis.rejected = 0;
        globalThis.ws = new WebSocket({url:?});
        ws.onopen = () => {{
            for (let i = 0; i < 9; i++) ws.flush().then(() => resolved++, () => rejected++);
            ws.close();
        }};
    "#
    ));
    tokio::time::timeout(Duration::from_secs(5), async {
        while !h.boolean("rejected === 9 && ws.readyState === WebSocket.CLOSED") {
            h.dispatch().await;
            while h._runtime.execute_pending_job().unwrap() {}
        }
    })
    .await
    .unwrap();
    assert!(h.boolean("resolved === 0"));
    server.await.unwrap();
}

#[test]
fn validates_url_protocols_origin_and_selection() {
    for url in [
        "https://host",
        "ws://",
        "ws://user:secret@host",
        "ws://host/#fragment",
        "file:///tmp/x",
    ] {
        assert!(request(url, &[], None).is_err(), "{url}");
    }
    for protocols in [
        vec![""],
        vec!["a b"],
        vec!["a,b"],
        vec!["é"],
        vec!["a", "a"],
        vec!["x\r\ny"],
    ] {
        let protocols = protocols.into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert!(request("wss://example.test/path?token=secret", &protocols, None).is_err());
    }
    for origin in [
        "https://host/",
        "https://host/path",
        "ws://host",
        "https://user:pass@host",
        "https://host?x",
        "https://host#x",
        "https://host\r\nx:y",
    ] {
        assert!(request("ws://host", &[], Some(origin)).is_err(), "{origin}");
    }
    assert!(request(
        "wss://example.test/path?secret=ok",
        &["proto.v2".into()],
        Some("https://client.test:8443")
    )
    .is_ok());
    assert!(request("ws://localhost", &[], Some("null")).is_ok());
    let mut response = Response::new(None);
    assert_eq!(selected_protocol(&response, &["v2".into()]).unwrap(), "");
    response.headers_mut().insert(
        header::SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_static("v2"),
    );
    assert_eq!(selected_protocol(&response, &["v2".into()]).unwrap(), "v2");
    assert!(selected_protocol(&response, &[]).is_err());
    response.headers_mut().append(
        header::SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_static("v2"),
    );
    assert!(selected_protocol(&response, &["v2".into()]).is_err());
}

#[tokio::test]
async fn quickjs_invalid_constructor_payload_and_queue_caps() {
    let h = Harness::new();
    h.eval(
        r#"
        function rejects(f) { let failed = false; try { f(); } catch (_) { failed = true; }
            if (!failed) throw Error('expected rejection'); }
        rejects(() => WebSocket('ws://localhost'));
        for (const args of [[], [8], ['http://localhost'], ['ws://localhost','bad protocol'],
            ['ws://localhost',['a','a']], ['ws://localhost',[4]], ['ws://localhost',null],
            ['ws://localhost',[],null], ['ws://localhost',[],{headers:{}}],
            ['ws://localhost',[],{origin:4}], ['ws://localhost',[],{origin:'https://x/path'}]])
            rejects(() => new WebSocket(...args));
    "#,
    );
    assert!(h.registry.lock().unwrap().is_empty());
    // A controlled open connection isolates the native send boundary without races.
    let (tx, mut rx) = mpsc::channel(QUEUE_CAP);
    let (action_tx, _) = mpsc::unbounded_channel();
    let connection = Arc::new(WsConnection::new(tx, action_tx));
    connection.state.store(OPEN, Ordering::SeqCst);
    h.registry.lock().unwrap().insert(50, connection);
    h.eval(
        r#"
        rejects(() => __tela_ws_send__(50, {}));
        rejects(() => __tela_ws_send__(50, [1,2]));
        rejects(() => __tela_ws_send__(50, new DataView(new ArrayBuffer(2))));
        rejects(() => __tela_ws_send__(50, new Uint16Array(2)));
        rejects(() => __tela_ws_send__(50, new Uint8Array(1048577)));
        rejects(() => __tela_ws_send__(50, 'é'.repeat(524289)));
        __tela_ws_send__(50, new Uint8Array(1048576));
        for (let i = 1; i < 8; i++) __tela_ws_send__(50, 'ok');
        rejects(() => __tela_ws_send__(50, 'overflow'));
        rejects(() => __tela_ws_send__(999, 'missing'));
    "#,
    );
    assert!(
        matches!(rx.try_recv().unwrap(), Outgoing::Message(Message::Binary(bytes)) if bytes.len() == MAX_PAYLOAD)
    );
    for _ in 1..QUEUE_CAP {
        assert!(matches!(
            rx.try_recv().unwrap(),
            Outgoing::Message(Message::Text(_))
        ));
    }
    assert!(rx.try_recv().is_err());
    h.eval("__tela_ws_send__(50, new ArrayBuffer(0)); __tela_ws_send__(50, new Uint8Array(0));");
    assert!(matches!(rx.try_recv().unwrap(), Outgoing::Message(message) if message.len() == 0));
    assert!(matches!(rx.try_recv().unwrap(), Outgoing::Message(message) if message.len() == 0));
    drop(rx);
    h.eval("rejects(() => __tela_ws_flush__(50, 1));");
    h.context.with(|ctx| {
        let mut buffer = ArrayBuffer::new_copy(ctx.clone(), [1u8, 2]).unwrap();
        let view = TypedArray::<u8>::from_arraybuffer(buffer.clone()).unwrap();
        buffer.detach();
        assert!(payload(&ctx, buffer.into_value()).is_err());
        ctx.catch();
        assert!(payload(&ctx, view.into_value()).is_err());
        ctx.catch();
    });
    shutdown_ws(&h.registry);
}

#[test]
fn bounded_inbound_one_wake_and_reserved_terminal_events() {
    let (outgoing, _) = mpsc::channel(QUEUE_CAP);
    let (action_tx, _) = mpsc::unbounded_channel();
    let connection = WsConnection::new(outgoing, action_tx);
    let (tx, mut rx) = mpsc::unbounded_channel();
    for _ in 0..QUEUE_CAP {
        assert!(connection.enqueue(1, Event::Binary(vec![0; MAX_PAYLOAD]), &tx));
    }
    for _ in 0..100 {
        assert!(!connection.enqueue(1, Event::Text("excess".into()), &tx));
    }
    connection.finish(1, Some("WebSocket receive queue full"), &tx);
    connection.finish(1, None, &tx);
    let queue = connection.incoming.lock().unwrap();
    assert_eq!(queue.events.len(), QUEUE_CAP + 2);
    assert!(queue.finished);
    let action = rx.try_recv().unwrap();
    assert_eq!(action["data"], "");
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn quickjs_drain_acknowledges_wake_and_handler_failure_still_cleans_up() {
    let h = Harness::new();
    let (listener, url) = listener().await;
    h.eval(&format!("globalThis.ws = new WebSocket({url:?});"));
    // Don't run the dial task yet; inject controlled terminal events into the queue.
    let connection = h.registry.lock().unwrap()[&1].clone();
    let (tx, mut rx) = mpsc::unbounded_channel();
    assert!(connection.enqueue(1, Event::Open("".into()), &tx));
    h.eval("__tela_ws_event__(1, 'drain', '');");
    assert_eq!(rx.try_recv().unwrap()["event"], "drain");
    assert!(connection.enqueue(1, Event::Binary(vec![1, 2, 3]), &tx));
    h.context.with(|ctx| {
        let batch = pull(ctx.clone(), &h.registry, 1).unwrap();
        let event: Object = batch.get(0).unwrap();
        let mut buffer: ArrayBuffer = event.get("data").unwrap();
        assert_eq!(buffer.as_bytes().unwrap(), &[1, 2, 3]);
        buffer.detach(); // received buffers must remain safe to finalize after detach
    });
    rx.try_recv().unwrap();
    assert!(connection.enqueue(1, Event::Text("hello".into()), &tx));
    connection.finish(1, Some("WebSocket transport failed"), &tx);
    h.eval(
        r#"
        globalThis.closed = false;
        ws.onmessage = () => {throw Error('user handler');};
        ws.onclose = () => {closed = true;};
        try { __tela_ws_event__(1, 'drain', ''); } catch (_) {}
        if (!closed || ws.readyState !== 3) throw Error('terminal event lost');
    "#,
    );
    assert!(h.registry.lock().unwrap().is_empty());
    assert!(rx.try_recv().is_ok());
    assert!(rx.try_recv().is_err());
    connection.cancel.cancel();
    drop(listener);
}

#[tokio::test]
async fn close_during_dial_and_connection_limit() {
    let (listener, url) = listener().await;
    let mut h = Harness::new();
    h.eval(&format!(
        r#"
        globalThis.sockets = [];
        globalThis.closed = 0;
        for (let i=0; i<8; i++) {{
            let ws = new WebSocket({url:?}, 'fixture.v2');
            ws.onclose = () => closed++;
            sockets.push(ws);
        }}
        let capped = false;
        try {{ new WebSocket({url:?}); }} catch (_) {{ capped = true; }}
        if (!capped) throw Error('connection limit');
        try {{ sockets[0].send('early'); throw Error('accepted early send'); }}
        catch (e) {{ if (!(e instanceof TypeError)) throw e; }}
        for (const ws of sockets) {{ ws.close(); ws.close(); }}
    "#
    ));
    h.until("closed === 8").await;
    assert!(h.boolean("sockets.every(ws => ws.readyState === 3)"));
    assert!(h.registry.lock().unwrap().is_empty());
    drop(listener);
}

#[tokio::test]
async fn shutdown_and_context_drop_cancel_owned_handshake() {
    for explicit in [true, false] {
        let (listener, url) = listener().await;
        let h = Harness::new();
        h.eval(&format!("globalThis.ws = new WebSocket({url:?});"));
        let (mut peer, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let registry = h.registry.clone();
        let connection = registry.lock().unwrap()[&1].clone();
        if explicit {
            shutdown_ws(&registry);
            shutdown_ws(&registry);
        }
        let Harness {
            context,
            _runtime: runtime,
            ..
        } = h;
        drop(context);
        // Explicit shutdown works even while the JS runtime remains alive. QuickJS
        // may defer native function finalizers until runtime destruction otherwise.
        if explicit {
            assert!(registry.lock().unwrap().is_empty());
        }
        drop(runtime);
        assert!(registry.lock().unwrap().is_empty());
        assert!(connection.cancel.is_cancelled());
        // Drain any HTTP request then observe TCP EOF; no server handshake response.
        use tokio::io::AsyncReadExt;
        tokio::time::timeout(Duration::from_secs(3), async {
            let mut bytes = [0; 2048];
            loop {
                match peer.read(&mut bytes).await {
                    Ok(0) | Err(_) => break,
                    _ => {}
                }
            }
        })
        .await
        .expect("dial socket leaked");
    }
}

#[tokio::test]
async fn shutdown_open_socket_drops_silent_reader() {
    let (listener, url) = listener().await;
    let server = tokio::spawn(async move {
        let (peer, _) = listener.accept().await.unwrap();
        let mut socket = accept_async(peer).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("open reader leaked");
    });
    let mut h = Harness::new();
    h.eval(&format!("globalThis.ws = new WebSocket({url:?});"));
    h.until("ws.readyState === WebSocket.OPEN").await;
    let connection = h.registry.lock().unwrap()[&1].clone();
    shutdown_ws(&h.registry);
    server.await.unwrap();
    assert!(h.registry.lock().unwrap().is_empty());
    assert!(connection.incoming.lock().unwrap().events.is_empty());
    assert!(h.actions.try_recv().is_err());
}

#[tokio::test]
async fn server_protocol_rejection_is_redacted_and_terminal() {
    let (listener, url) = listener().await;
    let server = tokio::spawn(async move {
        let (peer, _) = listener.accept().await.unwrap();
        let _ = accept_hdr_async(peer, |_: &Request, mut response: ServerResponse| {
            response.headers_mut().insert(
                header::SEC_WEBSOCKET_PROTOCOL,
                HeaderValue::from_static("secret-server-value"),
            );
            Ok(response)
        })
        .await;
    });
    let mut h = Harness::new();
    h.eval(&format!(
        r#"
        globalThis.closed = false; globalThis.errors = []; globalThis.opened = false;
        globalThis.ws = new WebSocket({url:?} + '/?credential=secret-client-value', 'offered');
        ws.onopen = () => {{opened = true;}};
        ws.onerror = e => errors.push(e.message);
        ws.onclose = () => {{closed = true;}};
    "#
    ));
    h.until("closed").await;
    assert!(h.boolean("!opened && errors.length === 1 && !errors[0].includes('secret') && !errors[0].includes('127.0.0.1')"));
    assert!(h.registry.lock().unwrap().is_empty());
    server.await.unwrap();
}

#[tokio::test]
async fn receive_oversize_and_queue_overflow_are_terminal() {
    for oversize in [true, false] {
        let (listener, url) = listener().await;
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (peer, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(peer).await.unwrap();
            if oversize {
                let _ = ws
                    .send(Message::Binary(vec![0; MAX_PAYLOAD + 1].into()))
                    .await;
            } else {
                for _ in 0..QUEUE_CAP + 4 {
                    if ws.send(Message::Binary(vec![7; 8].into())).await.is_err() {
                        break;
                    }
                }
            }
            let _ = done_tx.send(());
            let _ = tokio::time::timeout(Duration::from_secs(3), ws.next()).await;
        });
        let mut h = Harness::new();
        h.eval(&format!(
            r#"
            globalThis.closed = false; globalThis.errors = [];
            globalThis.ws = new WebSocket({url:?});
            ws.onerror = e => errors.push(e.message);
            ws.onclose = () => {{closed = true;}};
        "#
        ));
        let connection = h.registry.lock().unwrap()[&1].clone();
        tokio::time::timeout(Duration::from_secs(3), done_rx)
            .await
            .unwrap()
            .unwrap();
        // Wait for native termination WITHOUT draining its queue.
        tokio::time::timeout(Duration::from_secs(3), async {
            while connection.state.load(Ordering::SeqCst) != CLOSED {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(h.actions.len(), 1);
        assert!(connection.incoming.lock().unwrap().events.len() <= QUEUE_CAP + 2);
        h.until("closed").await;
        assert!(h.boolean("errors.length === 1"));
        assert!(h.registry.lock().unwrap().is_empty());
        server.await.unwrap();
    }
}

#[tokio::test]
async fn host_config_rejects_tls_downgrade() {
    let h = Harness::new();
    let (tx, _) = mpsc::unbounded_channel();
    h.context.with(|ctx| {
        assert!(register_websocket_with_config(
            &ctx,
            tx,
            h.registry.clone(),
            tokio::runtime::Handle::current(),
            WsConfig {
                connector: Some(Connector::Plain)
            }
        )
        .is_err());
    });
}
