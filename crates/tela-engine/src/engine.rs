use anyhow::{Context, Result, anyhow};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures_util::StreamExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use rquickjs::{AsyncContext, AsyncRuntime};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::elements::Element;
use crate::native::{
    register_clipboard, register_console, register_env, register_fetch, register_storage,
    register_timers, register_websocket, TimerHandles, WsSenders,
};

const H_AND_FRAGMENT: &str = r#"
globalThis.h = function h(tag, props, ...children) {
    var flatChildren = children
        .flat(Infinity)
        .filter(function(c) { return c != null && c !== false && c !== true; })
        .map(function(c) {
            return (typeof c === 'string' || typeof c === 'number')
                ? { tag: '__text__', props: { content: String(c) }, children: [] }
                : c;
        });
    if (typeof tag === 'function') {
        return tag(Object.assign({}, props, { children: flatChildren }));
    }
    return { tag: tag, props: props || {}, children: flatChildren };
};

globalThis.Fragment = function Fragment(props) {
    return { tag: '__fragment__', props: {}, children: props.children || [] };
};

globalThis.__tela_state__ = undefined;
globalThis.__tela_dispatch_queue__ = [];
globalThis.__tela_quit__ = false;

globalThis.Tela = {
    dispatch: function(action) {
        globalThis.__tela_dispatch_queue__.push(action);
    },
    quit: function() {
        globalThis.__tela_quit__ = true;
    },
    columns: 0,
    rows: 0,
};
"#;

const TELA_RUNTIME: &str = r#"
(function() {
    var userReduce = globalThis.reduce;
    var bindings = globalThis.keybindings;

    if (!bindings || !userReduce) return;

    globalThis.reduce = function(state, action) {
        if (action.type !== "__tela_key__") {
            return userReduce(state, action);
        }

        var mode = (state && state.mode) || "normal";
        var modeBindings = bindings[mode] || {};

        var keyName = action.key;
        if (action.ctrl) keyName = "Ctrl+" + keyName;
        if (action.alt) keyName = "Alt+" + keyName;
        if (action.shift && action.key.length > 1) keyName = "Shift+" + keyName;

        var actionName = modeBindings[keyName] || modeBindings[action.key];

        if (actionName) {
            if (actionName === "quit") {
                Tela.quit();
                return state;
            }
            return userReduce(state, { type: actionName });
        }

        if (mode === "insert") {
            if (action.ctrl && action.key === "j") {
                return userReduce(state, { type: "input_newline" });
            }
            if (!action.ctrl && !action.alt && action.key.length === 1) {
                return userReduce(state, { type: "input_char", char: action.key });
            }
            switch (action.key) {
                case "Backspace": return userReduce(state, { type: "input_backspace" });
                case "Enter": return userReduce(state, { type: "input_submit" });
                case "Escape": return userReduce(state, { type: "enter_normal" });
            }
        }

        return state;
    };
})();
"#;

pub struct Engine {
    rt: AsyncRuntime,
    ctx: AsyncContext,
    _action_tx: mpsc::UnboundedSender<serde_json::Value>,
    action_rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<serde_json::Value>>,
    _timer_handles: TimerHandles,
    _ws_senders: WsSenders,
}

impl Engine {
    pub async fn new() -> Result<Self> {
        let all_perms: Vec<String> = ["network", "storage", "clipboard", "env"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        Self::new_with_manifest("default", &all_perms).await
    }

    pub async fn new_with_manifest(app_name: &str, permissions: &[String]) -> Result<Self> {
        let rt = AsyncRuntime::new().context("failed to create QuickJS runtime")?;
        let ctx = AsyncContext::full(&rt)
            .await
            .context("failed to create QuickJS context")?;

        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let timer_handles: TimerHandles = Arc::new(Mutex::new(HashMap::new()));
        let ws_senders: WsSenders = Arc::new(Mutex::new(HashMap::new()));

        {
            let tx = action_tx.clone();
            let th = timer_handles.clone();
            let ws = ws_senders.clone();
            let rt_handle = tokio::runtime::Handle::current();
            let app_name = app_name.to_string();
            let perms: Vec<String> = permissions.to_vec();
            ctx.with(move |ctx| -> Result<()> {
                register_console(&ctx)?;
                ctx.eval::<(), _>(H_AND_FRAGMENT)
                    .map_err(|e| anyhow!("failed to register h/Fragment: {e}"))?;
                register_timers(&ctx, tx.clone(), th, rt_handle.clone())?;
                if perms.iter().any(|p| p == "network") {
                    register_fetch(&ctx, tx.clone(), rt_handle.clone())?;
                    register_websocket(&ctx, tx, ws, rt_handle)?;
                }
                if perms.iter().any(|p| p == "storage") {
                    register_storage(&ctx, &app_name)?;
                }
                if perms.iter().any(|p| p == "env") {
                    register_env(&ctx)?;
                }
                if perms.iter().any(|p| p == "clipboard") {
                    register_clipboard(&ctx)?;
                }
                Ok(())
            })
            .await?;
        }

        Ok(Engine {
            rt,
            ctx,
            _action_tx: action_tx,
            action_rx: tokio::sync::Mutex::new(action_rx),
            _timer_handles: timer_handles,
            _ws_senders: ws_senders,
        })
    }

    pub async fn load_bundle(&self, source: &str) -> Result<()> {
        let source = source.to_string();
        self.ctx
            .with(move |ctx| -> Result<()> {
                ctx.eval::<(), _>(source.as_str())
                    .map_err(|e| anyhow!("failed to evaluate bundle: {e}"))?;

                ctx.eval::<(), _>(
                    r#"
                    if (typeof initialState !== 'undefined') {
                        globalThis.__tela_state__ = (typeof initialState === 'function')
                            ? initialState()
                            : JSON.parse(JSON.stringify(initialState));
                    }
                    "#,
                )
                .map_err(|e| anyhow!("failed to capture initialState: {e}"))?;

                ctx.eval::<(), _>(TELA_RUNTIME)
                    .map_err(|e| anyhow!("failed to load tela runtime: {e}"))?;

                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn get_initial_state_json(&self) -> Result<serde_json::Value> {
        self.ctx
            .with(|ctx| -> Result<serde_json::Value> {
                let val: rquickjs::Value =
                    ctx.eval("globalThis.__tela_state__")
                        .map_err(|e| anyhow!("failed to get state: {e}"))?;
                js_value_to_json(&ctx, val)
            })
            .await
    }

    pub async fn reduce(&self, action: serde_json::Value) -> Result<()> {
        let action_json = serde_json::to_string(&action)?;
        self.ctx
            .with(move |ctx| -> Result<()> {
                let script = format!(
                    r#"
                    (function() {{
                        var action = JSON.parse('{}');
                        if (typeof reduce === 'function') {{
                            globalThis.__tela_state__ = reduce(globalThis.__tela_state__, action);
                        }}
                    }})();
                    "#,
                    action_json.replace('\\', "\\\\").replace('\'', "\\'")
                );
                ctx.eval::<(), _>(script.as_str())
                    .map_err(|e| anyhow!("reduce failed: {e}"))?;
                Ok(())
            })
            .await?;
        Ok(())
    }

    pub async fn view(&self) -> Result<Element> {
        self.ctx
            .with(|ctx| -> Result<Element> {
                let result: rquickjs::Value = ctx
                    .eval(
                        r#"
                        (function() {
                            globalThis.__tela_dispatch_queue__ = [];
                            var dispatch = function(action) {
                                globalThis.__tela_dispatch_queue__.push(action);
                            };
                            if (typeof view === 'function') {
                                return view(globalThis.__tela_state__, dispatch);
                            }
                            return { tag: 'text', props: { content: 'no view() exported' }, children: [] };
                        })()
                        "#,
                    )
                    .map_err(|e| anyhow!("view() failed: {e}"))?;

                let json_val = js_value_to_json(&ctx, result)?;
                serde_json::from_value(json_val).context("failed to parse element tree")
            })
            .await
    }

    pub async fn drain_dispatch_queue(&self) -> Result<Vec<serde_json::Value>> {
        self.ctx
            .with(|ctx| -> Result<Vec<serde_json::Value>> {
                let val: rquickjs::Value = ctx
                    .eval(
                        r#"
                        (function() {
                            var q = globalThis.__tela_dispatch_queue__ || [];
                            globalThis.__tela_dispatch_queue__ = [];
                            return q;
                        })()
                        "#,
                    )
                    .map_err(|e| anyhow!("failed to drain dispatch queue: {e}"))?;
                let json_val = js_value_to_json(&ctx, val)?;
                serde_json::from_value(json_val).context("dispatch queue is not an array")
            })
            .await
    }

    pub async fn execute_pending_jobs(&self) {
        for _ in 0..100 {
            match tokio::time::timeout(
                std::time::Duration::from_millis(1),
                self.rt.execute_pending_job(),
            )
            .await
            {
                Ok(Ok(true)) => continue,
                _ => break,
            }
        }
    }

    async fn fire_timer(&self, id: u64) -> Result<()> {
        let id = id as f64;
        self.ctx
            .with(move |ctx| -> Result<()> {
                let func: rquickjs::Function = ctx
                    .globals()
                    .get("__tela_fire_timer__")
                    .map_err(|e| anyhow!("__tela_fire_timer__ not found: {e}"))?;
                func.call::<_, ()>((id,))
                    .map_err(|e| anyhow!("fire_timer failed: {e}"))?;
                Ok(())
            })
            .await
    }

    async fn fire_ws_event(&self, id: u64, event: &str, data: &str) -> Result<()> {
        let id = id as f64;
        let event = event.to_string();
        let data = data.to_string();
        self.ctx
            .with(move |ctx| -> Result<()> {
                let func: rquickjs::Function = ctx
                    .globals()
                    .get("__tela_ws_event__")
                    .map_err(|e| anyhow!("__tela_ws_event__ not found: {e}"))?;
                func.call::<_, ()>((id, event, data))
                    .map_err(|e| anyhow!("fire_ws_event failed: {e}"))?;
                Ok(())
            })
            .await
    }

    async fn resolve_fetch(&self, id: u64, result_json: &str) -> Result<()> {
        let id = id as f64;
        let result_json = result_json.to_string();
        self.ctx
            .with(move |ctx| -> Result<()> {
                let func: rquickjs::Function = ctx
                    .globals()
                    .get("__tela_resolve_fetch__")
                    .map_err(|e| anyhow!("__tela_resolve_fetch__ not found: {e}"))?;
                func.call::<_, ()>((id, result_json))
                    .map_err(|e| anyhow!("resolve_fetch failed: {e}"))?;
                Ok(())
            })
            .await
    }

    async fn check_quit(&self) -> bool {
        self.ctx
            .with(|ctx| -> bool {
                ctx.eval::<bool, _>("globalThis.__tela_quit__ === true")
                    .unwrap_or(false)
            })
            .await
    }

    async fn update_terminal_size(&self) {
        if let Ok((cols, rows)) = crossterm::terminal::size() {
            let cols = cols as u32;
            let rows = rows as u32;
            self.ctx
                .with(move |ctx| {
                    let _ = ctx.eval::<(), _>(format!(
                        "globalThis.Tela.columns={cols};globalThis.Tela.rows={rows};"
                    ));
                })
                .await;
        }
    }

    async fn handle_internal_action(&self, action: serde_json::Value) -> Result<()> {
        match action.get("type").and_then(|v| v.as_str()) {
            Some("__tela_timer__") => {
                let id = action.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                self.fire_timer(id).await?;
            }
            Some("__tela_ws__") => {
                let id = action.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                let event = action.get("event").and_then(|v| v.as_str()).unwrap_or("");
                let data = action.get("data").and_then(|v| v.as_str()).unwrap_or("");
                self.fire_ws_event(id, event, data).await?;
            }
            Some("__tela_fetch__") => {
                let id = action.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
                let result = action.get("result").and_then(|v| v.as_str()).unwrap_or("{}");
                self.resolve_fetch(id, result).await?;
                self.execute_pending_jobs().await;
            }
            _ => {
                self.reduce(action).await?;
            }
        }

        let dispatched = self.drain_dispatch_queue().await?;
        for a in dispatched {
            self.reduce(a).await?;
        }
        self.execute_pending_jobs().await;

        Ok(())
    }

    pub async fn run(&self) -> Result<()> {
        enable_raw_mode().context("failed to enable raw mode")?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .context("failed to enter alternate screen")?;
        let _guard = TerminalGuard;

        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend).context("failed to create terminal")?;

        let mut action_rx = self.action_rx.lock().await;
        let mut event_stream = EventStream::new();

        self.update_terminal_size().await;

        loop {
            let tree = self.view().await?;
            terminal.draw(|frame| {
                let area = frame.area();
                crate::renderer::render_element(frame, area, &tree);
            })?;

            tokio::select! {
                event = event_stream.next() => {
                    match event {
                        Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                            if key.modifiers.contains(KeyModifiers::CONTROL)
                                && key.code == KeyCode::Char('c')
                            {
                                break;
                            }

                            let key_name = match key.code {
                                KeyCode::Char(' ') => Some(" ".to_string()),
                                KeyCode::Char(c) => Some(c.to_string()),
                                KeyCode::Enter => Some("Enter".to_string()),
                                KeyCode::Esc => Some("Escape".to_string()),
                                KeyCode::Backspace => Some("Backspace".to_string()),
                                KeyCode::Tab => Some("Tab".to_string()),
                                KeyCode::Up => Some("Up".to_string()),
                                KeyCode::Down => Some("Down".to_string()),
                                KeyCode::Left => Some("Left".to_string()),
                                KeyCode::Right => Some("Right".to_string()),
                                KeyCode::Home => Some("Home".to_string()),
                                KeyCode::End => Some("End".to_string()),
                                KeyCode::PageUp => Some("PageUp".to_string()),
                                KeyCode::PageDown => Some("PageDown".to_string()),
                                KeyCode::Insert => Some("Insert".to_string()),
                                KeyCode::Delete => Some("Delete".to_string()),
                                KeyCode::F(n) => Some(format!("F{n}")),
                                _ => None,
                            };

                            if let Some(name) = key_name {
                                self.reduce(serde_json::json!({
                                    "type": "__tela_key__",
                                    "key": name,
                                    "ctrl": key.modifiers.contains(KeyModifiers::CONTROL),
                                    "alt": key.modifiers.contains(KeyModifiers::ALT),
                                    "shift": key.modifiers.contains(KeyModifiers::SHIFT),
                                })).await?;
                            }
                        }
                        Some(Ok(Event::Resize(w, h))) => {
                            self.update_terminal_size().await;
                            self.reduce(serde_json::json!({
                                "type": "__tela_resize__",
                                "columns": w,
                                "rows": h,
                            })).await?;
                        }
                        None => break,
                        _ => {}
                    }
                }
                action = action_rx.recv() => {
                    if let Some(action) = action {
                        self.handle_internal_action(action).await?;
                    }
                }
            }

            let dispatched = self.drain_dispatch_queue().await?;
            for action in dispatched {
                self.reduce(action).await?;
            }
            self.execute_pending_jobs().await;

            if self.check_quit().await {
                break;
            }
        }

        Ok(())
    }
}

fn js_value_to_json<'js>(
    ctx: &rquickjs::Ctx<'js>,
    val: rquickjs::Value<'js>,
) -> Result<serde_json::Value> {
    if val.is_undefined() || val.is_null() {
        return Ok(serde_json::Value::Null);
    }
    let json_str = ctx
        .json_stringify(val)
        .map_err(|e| anyhow!("JSON.stringify failed: {e}"))?
        .ok_or_else(|| anyhow!("JSON.stringify returned undefined"))?;
    let s = json_str
        .to_string()
        .map_err(|e| anyhow!("string conversion failed: {e}"))?;
    serde_json::from_str(&s).context("failed to parse JSON from JS")
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            std::io::stdout(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
    }
}
