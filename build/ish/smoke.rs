// Standalone verifier, never built as an SDK example/test (those enable ring).
use std::{io::{Read, Write}, sync::{Arc, Barrier, mpsc}, time::Duration};
use tela_engine::Engine;

fn contended_handoff() {
    let data = Arc::new(async_lock::Mutex::new((0usize, 0usize)));
    let gate = Arc::new(Barrier::new(5));
    let (tx, rx) = mpsc::channel();
    let held = data.lock_blocking();
    let mut threads = Vec::new();
    for _ in 0..4 {
        let data = data.clone();
        let gate = gate.clone();
        let tx = tx.clone();
        threads.push(std::thread::spawn(move || {
            assert!(data.try_lock().is_none());
            gate.wait();
            for _ in 0..1000 {
                let mut guard = data.lock_blocking();
                assert_eq!(guard.1, guard.0 * 3);
                guard.0 += 1;
                guard.1 = guard.0 * 3;
                drop(guard);
                std::thread::yield_now();
            }
            tx.send(()).unwrap();
        }));
    }
    gate.wait();
    // Force waiters through the notification slow path before first unlock.
    std::thread::sleep(Duration::from_millis(50));
    drop(held);
    for _ in 0..4 { rx.recv_timeout(Duration::from_secs(30)).expect("lost wake/deadlock"); }
    for thread in threads { thread.join().unwrap(); }
    assert_eq!(*data.lock_blocking(), (4000, 12000));
    println!("PASS: contended async-lock protected-data handoff (4 x 1000), no lost wakes");
}

async fn settle(engine: &Engine) {
    tokio::time::timeout(Duration::from_secs(180), async {
        loop {
            engine.pump_events().await.unwrap();
            if engine.get_initial_state_json().await.unwrap()["done"] == true { break; }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }).await.expect("Engine completion timeout");
    let state = engine.get_initial_state_json().await.unwrap();
    assert_eq!(state["ok"], true, "{state}");
}

async fn tls_fetch(cert: &str, key: &str, host: &str, expected: bool) {
    let identity = native_tls::Identity::from_pkcs8(
        &std::fs::read(cert).unwrap(), &std::fs::read(key).unwrap()).unwrap();
    let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(20))).unwrap();
        if let Ok(mut stream) = acceptor.accept(stream) {
            let mut request = [0; 4096];
            if stream.read(&mut request).unwrap_or(0) > 0 {
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nfixture-ok");
            }
        }
    });
    let engine = Engine::new_with_manifest("ish-tls-fixture", &["network".into()]).await.unwrap();
    engine.load_bundle(&format!(r#"
        var initialState={{done:false}};
        function reduce(s,a){{return a;}}
        fetch('https://{host}:{port}/').then(r=>{{
            Tela.dispatch({{done:true,ok:{expected}&&r.ok&&r.text()==='fixture-ok'}});
        }},e=>Tela.dispatch({{done:true,ok:!{expected}}}));
    "#)).await.unwrap();
    settle(&engine).await;
    engine.shutdown();
    server.join().unwrap();
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    assert_eq!(std::mem::size_of::<usize>(), 4);
    contended_handoff();
    let engine = Engine::new_with_manifest("ish-offline-smoke", &["crypto".into()]).await.unwrap();
    engine.load_bundle(r#"
        var initialState={done:false};
        function reduce(s,a){return a;}
        function view(s){return h('text',{},'café 日本語 🦀');}
        function hex(b){return Array.from(b,x=>x.toString(16).padStart(2,'0')).join('');}
        var c=Tela.crypto;
        var utf='café 日本語 🦀';
        if(JSON.parse(JSON.stringify(utf))!==utf||Array.from(utf).length!==10)throw Error('Unicode');
        if(hex(c.sha256(new Uint8Array([97,98,99])))!=='ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad')throw Error('SHA-256');
        var enc=new TextEncoder(),dec=new TextDecoder('utf-8',{fatal:true});
        if(dec.decode(enc.encode(utf))!==utf)throw Error('UTF-8');
        var a=c.x25519.generate(),b=c.x25519.generate();
        var x=c.x25519.derive(a,c.x25519.publicKey(b)),y=c.x25519.derive(b,c.x25519.publicKey(a));
        if(!c.timingSafeEqual(x,y))throw Error('X25519');
        x.fill(0);y.fill(0);a.dispose();b.dispose();
        var raw=c.randomBytes(32),key=c.chacha20poly1305.importKey32(raw);raw.fill(0);
        var nonce=new Uint8Array(12),aad=enc.encode('fixture'),plain=enc.encode('offline check');
        var ct=c.chacha20poly1305.seal(key,nonce,aad,plain);
        if(!c.timingSafeEqual(plain,c.chacha20poly1305.open(key,nonce,aad,ct)))throw Error('AEAD');
        ct[0]^=1;var rejected=false;
        try{c.chacha20poly1305.open(key,nonce,aad,ct);}catch(e){rejected=true;}
        if(!rejected)throw Error('AEAD accepted tampering');key.dispose();
        var start=Date.now();
        var job=c.argon2id(enc.encode('tela-artifact-check'),new Uint8Array([0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15]),{version:19,memoryKiB:65536,passes:3,parallelism:1,outputLength:80});
        job.result.then(bytes=>{var n=bytes.length;bytes.fill(0);Tela.dispatch({done:true,ok:n===80,argonMs:Date.now()-start});},e=>Tela.dispatch({done:true,ok:false,error:String(e)}));
    "#).await.unwrap();
    settle(&engine).await;
    engine.view().await.unwrap();
    println!("PASS: public Engine QuickJS/UTF-8/RNG/SHA-256/X25519/AEAD+tamper rejection; Argon2id 64MiB/3 passes/80 bytes: {}", engine.get_initial_state_json().await.unwrap());
    engine.shutdown();
    let root = std::env::var("ISH_TLS_FIXTURES").expect("fixture path");
    tls_fetch(&format!("{root}/server.pem"), &format!("{root}/server.key"), "localhost", true).await;
    println!("PASS: Engine fetch native TLS accepts private SSL_CERT_FILE CA and matching hostname");
    tls_fetch(&format!("{root}/server.pem"), &format!("{root}/server.key"), "127.0.0.1", false).await;
    println!("PASS: Engine fetch native TLS rejects hostname mismatch");
    tls_fetch(&format!("{root}/untrusted.pem"), &format!("{root}/untrusted.key"), "localhost", false).await;
    println!("PASS: Engine fetch native TLS rejects untrusted certificate");
}
