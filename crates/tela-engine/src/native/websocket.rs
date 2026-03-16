use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use rquickjs::Ctx;
use tokio::sync::mpsc;

pub type WsSenders = Arc<Mutex<HashMap<u64, mpsc::UnboundedSender<String>>>>;

const WS_BOOTSTRAP: &str = r#"
globalThis.__tela_ws_instances__ = {};
globalThis.__tela_next_ws_id__ = 1;

globalThis.__tela_ws_event__ = function(id, eventType, data) {
    var ws = globalThis.__tela_ws_instances__[id];
    if (!ws) return;
    switch (eventType) {
        case "open":
            ws.readyState = 1;
            if (ws.onopen) ws.onopen();
            break;
        case "message":
            if (ws.onmessage) ws.onmessage({ data: data });
            break;
        case "close":
            ws.readyState = 3;
            if (ws.onclose) ws.onclose();
            delete globalThis.__tela_ws_instances__[id];
            break;
        case "error":
            if (ws.onerror) ws.onerror({ message: data });
            break;
    }
};

globalThis.WebSocket = function(url) {
    var self = this;
    self.url = url;
    self.readyState = 0;
    self.onopen = null;
    self.onmessage = null;
    self.onclose = null;
    self.onerror = null;

    var id = globalThis.__tela_next_ws_id__++;
    self._id = id;
    globalThis.__tela_ws_instances__[id] = self;

    __tela_ws_connect__(id, url);

    self.send = function(data) {
        __tela_ws_send__(id, typeof data === "string" ? data : JSON.stringify(data));
    };

    self.close = function() {
        __tela_ws_close__(id);
    };
};
"#;

pub fn register_websocket(
    ctx: &Ctx<'_>,
    action_tx: mpsc::UnboundedSender<serde_json::Value>,
    ws_senders: WsSenders,
    handle: tokio::runtime::Handle,
) -> anyhow::Result<()> {
    {
        let tx = action_tx.clone();
        let senders = ws_senders.clone();
        let rt_handle = handle.clone();
        let connect = move |id: u64, url: String| {
            let tx = tx.clone();
            let senders = senders.clone();
            rt_handle.spawn(async move {
                match tokio_tungstenite::connect_async(&url).await {
                    Ok((ws_stream, _)) => {
                        let _ = tx.send(serde_json::json!({
                            "type": "__tela_ws__", "id": id, "event": "open", "data": ""
                        }));

                        let (mut write, mut read) = ws_stream.split();
                        let (msg_tx, mut msg_rx) = mpsc::unbounded_channel::<String>();

                        if let Ok(mut map) = senders.lock() {
                            map.insert(id, msg_tx);
                        }

                        // Write task
                        let write_task = tokio::spawn(async move {
                            while let Some(msg) = msg_rx.recv().await {
                                if write
                                    .send(tokio_tungstenite::tungstenite::Message::Text(
                                        msg.into(),
                                    ))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        });

                        // Read loop
                        while let Some(msg) = read.next().await {
                            match msg {
                                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                                    let _ = tx.send(serde_json::json!({
                                        "type": "__tela_ws__", "id": id, "event": "message", "data": text.to_string()
                                    }));
                                }
                                Ok(tokio_tungstenite::tungstenite::Message::Close(_)) => break,
                                Err(e) => {
                                    let _ = tx.send(serde_json::json!({
                                        "type": "__tela_ws__", "id": id, "event": "error", "data": e.to_string()
                                    }));
                                    break;
                                }
                                _ => {}
                            }
                        }

                        // Cleanup
                        if let Ok(mut map) = senders.lock() {
                            map.remove(&id);
                        }
                        write_task.abort();

                        let _ = tx.send(serde_json::json!({
                            "type": "__tela_ws__", "id": id, "event": "close", "data": ""
                        }));
                    }
                    Err(e) => {
                        let _ = tx.send(serde_json::json!({
                            "type": "__tela_ws__", "id": id, "event": "error", "data": e.to_string()
                        }));
                    }
                }
            });
        };

        ctx.globals().set(
            "__tela_ws_connect__",
            rquickjs::Function::new(ctx.clone(), connect)?,
        )?;
    }

    // ws_send
    {
        let senders = ws_senders.clone();
        let send = move |id: u64, data: String| {
            if let Ok(map) = senders.lock() {
                if let Some(tx) = map.get(&id) {
                    let _ = tx.send(data);
                }
            }
        };

        ctx.globals().set(
            "__tela_ws_send__",
            rquickjs::Function::new(ctx.clone(), send)?,
        )?;
    }

    // ws_close
    {
        let senders = ws_senders.clone();
        let close = move |id: u64| {
            if let Ok(mut map) = senders.lock() {
                map.remove(&id); // dropping the sender closes the write channel
            }
        };

        ctx.globals().set(
            "__tela_ws_close__",
            rquickjs::Function::new(ctx.clone(), close)?,
        )?;
    }

    ctx.eval::<(), _>(WS_BOOTSTRAP)
        .map_err(|e| anyhow::anyhow!("failed to register websocket bootstrap: {e}"))?;

    Ok(())
}
