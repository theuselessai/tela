// Kept separate from the runtime implementation so routing remains externally observable.
#[cfg(test)]
mod tests {
    use crate::{Engine, NativeModule};
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };
    use std::time::Duration;

    async fn settle(engine: &Engine) {
        for _ in 0..50 {
            engine.execute_pending_jobs().await;
            if engine.get_initial_state_json().await.unwrap()["done"] == true {
                return;
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }

    #[tokio::test]
    async fn catch_all_key_binding_preserves_modifiers_and_explicit_precedence() {
        let engine=Engine::new_with_manifest("raw-key-fixture",&[]).await.unwrap();
        engine.load_bundle("var initialState={mode:'custom'};var keybindings={custom:{Enter:'submit','*':'raw_key'}};function reduce(s,a){return {mode:'custom',last:a};}").await.unwrap();
        engine.reduce_and_process(serde_json::json!({"type":"__tela_key__","key":"b","ctrl":true,"alt":false,"shift":false})).await.unwrap();
        let last=engine.get_initial_state_json().await.unwrap()["last"].clone();
        assert_eq!(last,serde_json::json!({"type":"raw_key","key":"b","ctrl":true,"alt":false,"shift":false}));
        engine.reduce_and_process(serde_json::json!({"type":"__tela_key__","key":"*","ctrl":false,"alt":false,"shift":true})).await.unwrap();
        assert_eq!(engine.get_initial_state_json().await.unwrap()["last"]["key"],"*");
        engine.reduce_and_process(serde_json::json!({"type":"__tela_key__","key":"Enter","ctrl":false,"alt":false,"shift":false})).await.unwrap();
        assert_eq!(engine.get_initial_state_json().await.unwrap()["last"],serde_json::json!({"type":"submit"}));
        engine.shutdown();
    }

    #[tokio::test]
    async fn builtin_credentials_uses_private_host_root_and_named_profile() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let root = std::env::temp_dir().join(format!(
            "tela-store-engine-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let identity = format!("remote-{}", "a".repeat(64));
        let engine = Engine::new_with_options(
            &identity,
            &["credentials".into()],
            Vec::new(),
            crate::EngineOptions {
                credentials_root: Some(root.join("provider")),
                ..crate::EngineOptions::default()
            },
        )
        .await
        .unwrap();
        engine.load_bundle(r#"
            var initialState={done:false};function reduce(s,a){return a;}
            (async()=>{try{
                await Tela.credentials.prepare();
                var absent=await Tela.credentials.load('profile');
                await Tela.credentials.saveNew('profile',{fixture:true});
                var duplicate=false;try{await Tela.credentials.saveNew('profile',{fixture:false})}catch(e){duplicate=true;}
                var value=await Tela.credentials.load('profile');
                Tela.dispatch({done:true,ok:absent===null&&duplicate&&value.fixture===true});
            }catch(e){Tela.dispatch({done:true,ok:false});}})();
        "#).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            while engine.get_initial_state_json().await.unwrap()["done"] != true {
                engine.pump_events().await.unwrap();
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(engine.get_initial_state_json().await.unwrap()["ok"], true);
        let path = root.join("provider").join(identity).join("profile");
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        engine.shutdown();
    }

    #[tokio::test]
    async fn builtin_crypto_permission_and_async_completion() {
        let _serial = crate::native::crypto::ARGON_TEST_SERIAL.lock().await;
        let denied = Engine::new_with_manifest("crypto-denied", &[])
            .await
            .unwrap();
        denied
            .load_bundle("var initialState={present:typeof Tela.crypto!=='undefined'};")
            .await
            .unwrap();
        assert_eq!(
            denied.get_initial_state_json().await.unwrap()["present"],
            false
        );
        let engine = Engine::new_with_manifest("crypto-fixture", &["crypto".into()])
            .await
            .unwrap();
        engine.load_bundle(r#"
            var initialState={done:false};
            function reduce(s,a){return a.type==='complete'?{done:true,len:a.len}:s;}
            var job=Tela.crypto.argon2id(new Uint8Array([1]),new Uint8Array(8),{version:19,memoryKiB:8,passes:1,parallelism:1,outputLength:32});
            job.result.then(bytes=>Tela.dispatch({type:'complete',len:bytes.length}));
        "#).await.unwrap();
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                engine.pump_events().await.unwrap();
                if engine.get_initial_state_json().await.unwrap()["done"] == true {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(engine.get_initial_state_json().await.unwrap()["len"], 32);
        engine.shutdown();
        engine.load_bundle("var initialState={rejected:false};try{Tela.crypto.randomBytes(1)}catch(e){initialState.rejected=true}").await.unwrap();
        assert_eq!(
            engine.get_initial_state_json().await.unwrap()["rejected"],
            true
        );
    }

    #[tokio::test]
    async fn timer_promise_dispatch_is_reduced_before_render() {
        let engine = Engine::new_with_manifest("promise-fixture", &[])
            .await
            .unwrap();
        engine.load_bundle("var initialState={done:false};function reduce(s,a){return {done:true};}function view(){return h('text',{},'fixture');}setTimeout(()=>Promise.resolve().then(()=>Tela.dispatch({type:'done'})),1);").await.unwrap();
        tokio::time::timeout(Duration::from_secs(2), async {
            while engine.get_initial_state_json().await.unwrap()["done"] != true {
                engine.pump_events().await.unwrap();
                engine.view().await.unwrap();
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn shutdown_and_drop_close_pending_websocket_handshake() {
        use tokio::io::AsyncReadExt;
        for explicit_shutdown in [true, false] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let engine = Engine::new_with_manifest("ws-lifecycle-fixture", &["network".into()])
                .await
                .unwrap();
            engine
                .load_bundle(&format!(
                    "var initialState={{}};var socket=new WebSocket('ws://{address}/pending');"
                ))
                .await
                .unwrap();
            let (mut connection, _) =
                tokio::time::timeout(Duration::from_secs(2), listener.accept())
                    .await
                    .unwrap()
                    .unwrap();
            if explicit_shutdown {
                engine.shutdown();
            }
            drop(engine);
            let mut request = Vec::new();
            tokio::time::timeout(Duration::from_secs(2), connection.read_to_end(&mut request))
                .await
                .expect("owned dial socket leaked")
                .unwrap();
        }
    }

    #[tokio::test]
    async fn terminal_native_base64_and_modes_are_available_to_javascript() {
        let engine = Engine::new().await.unwrap();
        engine
            .load_bundle(
                r#"
            var id = Tela.terminal.create({ cols: 320, rows: 160 });
            Tela.terminal.writeBase64(id, "G1s/MWgbPRtbPzIwMDRo");
            var initialState = { modes: Tela.terminal.modes(id) };
            Tela.terminal.reset(id);
            initialState.afterReset = Tela.terminal.modes(id);
        "#,
            )
            .await
            .unwrap();
        let state = engine.get_initial_state_json().await.unwrap();
        assert_eq!(
            state["modes"],
            serde_json::json!({
                "applicationCursor": true,
                "applicationKeypad": true,
                "bracketedPaste": true,
            })
        );
        assert_eq!(
            state["afterReset"],
            serde_json::json!({
                "applicationCursor": false,
                "applicationKeypad": false,
                "bracketedPaste": false,
            })
        );
    }

    #[tokio::test]
    async fn native_module_success_and_permission_gate() {
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = calls.clone();
        let engine = Engine::new_with_native_modules(
            "test",
            &vec!["remote".into()],
            vec![NativeModule::new(
                "remote",
                "remote",
                move |operation, params, _| {
                    let calls = handler_calls.clone();
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        Ok(serde_json::json!({"operation": operation, "params": params}))
                    }
                },
            )],
        )
        .await
        .unwrap();
        engine
            .load_bundle(
                r#"
            var initialState = { has: Tela.native.has('remote') };
            Tela.native.call('remote', 'status', { ok: true }).then(function(v) {
                globalThis.__tela_state__.value = v; globalThis.__tela_state__.done = true;
            });
        "#,
            )
            .await
            .unwrap();
        settle(&engine).await;
        let state = engine.get_initial_state_json().await.unwrap();
        assert_eq!(state["has"], true);
        assert_eq!(
            state["value"],
            serde_json::json!({"operation":"status","params":{"ok":true}})
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let denied = Engine::new_with_native_modules(
            "test",
            &[],
            vec![NativeModule::new("remote", "remote", |_, _, _| async {
                panic!("denied module must not execute")
            })],
        )
        .await
        .unwrap();
        denied
            .load_bundle(
                r#"
            var initialState = {};
            Tela.native.call('remote', 'status', {}).catch(function(e) {
                globalThis.__tela_state__.error = e.message; globalThis.__tela_state__.done = true;
            });
        "#,
            )
            .await
            .unwrap();
        settle(&denied).await;
        assert_eq!(
            denied.get_initial_state_json().await.unwrap()["error"],
            "native permission denied"
        );
    }

    #[tokio::test]
    async fn native_completion_applies_dispatched_actions_before_render() {
        let engine = Engine::new_with_native_modules(
            "test",
            &["remote".into()],
            vec![NativeModule::new("remote", "remote", |_, _, _| async {
                Ok(serde_json::json!({"paired":true}))
            })],
        )
        .await
        .unwrap();
        engine.load_bundle(r#"
            var initialState = { done: false };
            function reduce(s, a) { return a.type === 'connected' ? { done: true } : s; }
            function view(s) { return h('text', {}, s.done ? 'Connected' : 'Waiting'); }
            Tela.native.call('remote', 'status', {}).then(function() { Tela.dispatch({type:'connected'}); });
        "#).await.unwrap();
        settle(&engine).await;
        assert_eq!(engine.get_initial_state_json().await.unwrap()["done"], true);
        engine.view().await.unwrap();
        assert_eq!(engine.get_initial_state_json().await.unwrap()["done"], true);
    }

    #[tokio::test]
    async fn native_module_rejects_invalid_bounds_and_overload_without_extra_execution() {
        let calls = Arc::new(AtomicUsize::new(0));
        let handler_calls = calls.clone();
        let engine = Engine::new_with_native_modules(
            "test",
            &vec!["remote".into()],
            vec![NativeModule::new(
                "remote",
                "remote",
                move |_, _, cancellation| {
                    let calls = handler_calls.clone();
                    async move {
                        calls.fetch_add(1, Ordering::SeqCst);
                        cancellation.cancelled().await;
                        Ok(serde_json::Value::Null)
                    }
                },
            )],
        )
        .await
        .unwrap();
        engine.load_bundle(r#"
            var initialState = { errors: [], done: false };
            Tela.native.call('missing', 'x', {}).catch(function(e) { globalThis.__tela_state__.errors.push(e.message); });
            Tela.native.call('remote', 'x', 'x'.repeat(65537)).catch(function(e) { globalThis.__tela_state__.errors.push(e.message); });
            var circular = {}; circular.self = circular;
            Tela.native.call('remote', 'x', circular).catch(function(e) { globalThis.__tela_state__.errors.push(e.message); });
            for (var i = 0; i < 9; i++) Tela.native.call('remote', 'x', {}).catch(function(e) { globalThis.__tela_state__.errors.push(e.message); });
            Promise.resolve().then(function() { globalThis.__tela_state__.done = true; });
        "#).await.unwrap();
        settle(&engine).await;
        let state = engine.get_initial_state_json().await.unwrap();
        assert!(state["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "native module not found"));
        assert!(state["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "native module params too large"));
        assert!(state["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "native module params invalid"));
        assert!(state["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "native module overloaded"));
        for _ in 0..20 {
            if calls.load(Ordering::SeqCst) == 8 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(calls.load(Ordering::SeqCst), 8);
    }

    #[tokio::test]
    async fn native_module_drop_cancels_inflight_handler() {
        // Handler deliberately ignores its cancellation token. Engine teardown
        // must still drop the pending future and its owned resources.
        struct DropSignal(Arc<AtomicBool>);
        impl Drop for DropSignal {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }
        let started = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));
        let handler_started = started.clone();
        let handler_cancelled = cancelled.clone();
        let engine = Engine::new_with_native_modules(
            "test",
            &vec!["remote".into()],
            vec![NativeModule::new(
                "remote",
                "remote",
                move |_, _, cancellation| {
                    let started = handler_started.clone();
                    let cancelled = handler_cancelled.clone();
                    async move {
                        let _guard = DropSignal(cancelled);
                        let _ = cancellation;
                        started.store(true, Ordering::SeqCst);
                        std::future::pending::<()>().await;
                        Ok(serde_json::Value::Null)
                    }
                },
            )],
        )
        .await
        .unwrap();
        engine
            .load_bundle("Tela.native.call('remote', 'wait', {}); var initialState = {};")
            .await
            .unwrap();
        for _ in 0..20 {
            if started.load(Ordering::SeqCst) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(started.load(Ordering::SeqCst));
        drop(engine);
        for _ in 0..20 {
            if cancelled.load(Ordering::SeqCst) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        assert!(cancelled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn terminal_mode_prefers_local_binding_then_forwards_raw_key() {
        let engine = Engine::new().await.unwrap();
        engine.load_bundle(r#"
            var initialState = { mode: 'terminal', seen: [] };
            var keybindings = { terminal: { 'Ctrl+]': 'prefix' } };
            function reduce(s, a) {
              if (a.type === 'prefix') return { mode: s.mode, seen: ['prefix'] };
              if (a.type === 'terminal_key') return { mode: s.mode, seen: [a.key, a.ctrl, a.alt, a.shift] };
              return s;
            }
        "#).await.unwrap();
        engine.reduce_and_process(serde_json::json!({ "type": "__tela_key__", "key": "]", "ctrl": true, "alt": false, "shift": false })).await.unwrap();
        assert_eq!(
            engine.get_initial_state_json().await.unwrap()["seen"][0],
            "prefix"
        );
        engine.reduce_and_process(serde_json::json!({ "type": "__tela_key__", "key": "Escape", "ctrl": false, "alt": false, "shift": false })).await.unwrap();
        let state = engine.get_initial_state_json().await.unwrap();
        assert_eq!(
            state["seen"],
            serde_json::json!(["Escape", false, false, false])
        );
    }
}
