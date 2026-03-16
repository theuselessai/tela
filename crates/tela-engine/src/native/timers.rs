use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rquickjs::Ctx;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub type TimerHandles = Arc<Mutex<HashMap<u64, JoinHandle<()>>>>;

const TIMER_BOOTSTRAP: &str = r#"
globalThis.__tela_timers__ = {};
globalThis.__tela_next_timer_id__ = 1;

globalThis.__tela_fire_timer__ = function(id) {
    var entry = globalThis.__tela_timers__[id];
    if (entry) {
        entry.callback();
        if (!entry.repeat) {
            delete globalThis.__tela_timers__[id];
        }
    }
};

globalThis.setTimeout = function(callback, ms) {
    var id = globalThis.__tela_next_timer_id__++;
    globalThis.__tela_timers__[id] = { callback: callback, repeat: false };
    __tela_start_timer__(id, ms || 0, false);
    return id;
};

globalThis.setInterval = function(callback, ms) {
    var id = globalThis.__tela_next_timer_id__++;
    globalThis.__tela_timers__[id] = { callback: callback, repeat: true };
    __tela_start_timer__(id, ms || 0, true);
    return id;
};

globalThis.clearTimeout = function(id) {
    if (id === undefined || id === null) return;
    delete globalThis.__tela_timers__[id];
    __tela_cancel_timer__(id);
};

globalThis.clearInterval = globalThis.clearTimeout;
"#;

pub fn register_timers(
    ctx: &Ctx<'_>,
    action_tx: mpsc::UnboundedSender<serde_json::Value>,
    timer_handles: TimerHandles,
    handle: tokio::runtime::Handle,
) -> anyhow::Result<()> {
    let tx = action_tx.clone();
    let handles = timer_handles.clone();

    let start_timer = {
        let tx = tx.clone();
        let handles = handles.clone();
        let rt_handle = handle.clone();
        move |id: u64, ms: u64, repeat: bool| {
            let tx = tx.clone();
            let handles = handles.clone();
            let handle = rt_handle.spawn(async move {
                if repeat {
                    let mut interval = tokio::time::interval(Duration::from_millis(ms.max(1)));
                    interval.tick().await; // first tick is immediate, skip it
                    loop {
                        interval.tick().await;
                        if tx
                            .send(serde_json::json!({"type": "__tela_timer__", "id": id}))
                            .is_err()
                        {
                            break;
                        }
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(ms)).await;
                    let _ = tx.send(serde_json::json!({"type": "__tela_timer__", "id": id}));
                }
            });
            if let Ok(mut map) = handles.lock() {
                map.insert(id, handle);
            };
        }
    };

    ctx.globals().set(
        "__tela_start_timer__",
        rquickjs::Function::new(ctx.clone(), start_timer)?,
    )?;

    let cancel_handles = timer_handles.clone();
    let cancel_timer = move |id: u64| {
        if let Ok(mut map) = cancel_handles.lock() {
            if let Some(handle) = map.remove(&id) {
                handle.abort();
            }
        }
    };

    ctx.globals().set(
        "__tela_cancel_timer__",
        rquickjs::Function::new(ctx.clone(), cancel_timer)?,
    )?;

    ctx.eval::<(), _>(TIMER_BOOTSTRAP)
        .map_err(|e| anyhow::anyhow!("failed to register timer bootstrap: {e}"))?;

    Ok(())
}
