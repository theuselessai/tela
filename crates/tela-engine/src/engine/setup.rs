//! Constructors and lifecycle glue, kept separate from the event loop.
use super::*;
use crate::native::websocket::{register_websocket_with_config, WsConfig};

/// Trusted embedding configuration, never populated from a downloaded manifest.
#[derive(Default)]
pub struct EngineOptions {
    pub websocket: WsConfig,
    /// Optional private provider root; app identity is still appended/validated.
    /// Useful for isolated embedding/test profiles. None uses platform data storage.
    pub credentials_root: Option<std::path::PathBuf>,
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
        Self::new_with_native_modules(app_name, permissions, Vec::new()).await
    }
    pub async fn new_with_native_modules(
        app_name: &str,
        permissions: &[String],
        modules: Vec<NativeModule>,
    ) -> Result<Self> {
        Self::new_with_websocket_config(app_name, permissions, modules, WsConfig::default()).await
    }
    /// Host-only network configuration, e.g. a private CA for isolated tests.
    /// Downloaded JS cannot supply TLS connectors or disable validation.
    pub async fn new_with_websocket_config(
        app_name: &str,
        permissions: &[String],
        modules: Vec<NativeModule>,
        websocket_config: WsConfig,
    ) -> Result<Self> {
        Self::new_with_options(
            app_name,
            permissions,
            modules,
            EngineOptions {
                websocket: websocket_config,
                ..EngineOptions::default()
            },
        )
        .await
    }

    pub async fn new_with_options(
        app_name: &str,
        permissions: &[String],
        mut modules: Vec<NativeModule>,
        options: EngineOptions,
    ) -> Result<Self> {
        if permissions.iter().any(|p| p == "credentials") {
            modules.push(match options.credentials_root {
                Some(root) => crate::native::credentials::module_at(app_name, root)?,
                None => crate::native::credentials::credentials_module(app_name)?,
            });
        }
        let websocket_config = options.websocket;
        let (native_runtime, native_completion_rx) = NativeModuleRuntime::new(modules)?;
        let rt = AsyncRuntime::new().context("failed to create QuickJS runtime")?;
        let ctx = AsyncContext::full(&rt)
            .await
            .context("failed to create QuickJS context")?;
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let timer_handles: TimerHandles = Arc::new(Mutex::new(HashMap::new()));
        let ws_senders: WsSenders = Arc::new(Mutex::new(HashMap::new()));
        let terminals = new_terminal_handles();
        let crypto = {
            let terminal_handles = terminals.clone();
            let tx = action_tx.clone();
            let th = timer_handles.clone();
            let ws = ws_senders.clone();
            let rt_handle = tokio::runtime::Handle::current();
            let app_name = app_name.to_string();
            let perms: Vec<String> = permissions.to_vec();
            let native_runtime = native_runtime.clone();
            ctx.with(
                move |ctx| -> Result<Option<Arc<crate::native::crypto::CryptoRuntime>>> {
                    register_console(&ctx)?;
                    register_encoding(&ctx)?;
                crate::native::clock::register_clock(&ctx)?;
                    ctx.eval::<(), _>(H_AND_FRAGMENT)
                        .map_err(|e| anyhow!("failed to register h/Fragment: {e}"))?;
                    crate::native::url::register_url(&ctx)?;
                    register_native_modules(&ctx, native_runtime.clone(), perms.clone())?;
                    let crypto = if perms.iter().any(|p| p == "crypto") {
                        Some(crate::native::crypto::register_crypto(&ctx)?)
                    } else {
                        None
                    };
                    if perms.iter().any(|p| p == "credentials") {
                        crate::native::credentials::register_credentials(&ctx)?;
                    }
                    register_terminal(&ctx, terminal_handles.clone())?;
                    register_timers(&ctx, tx.clone(), th, rt_handle.clone())?;
                    if perms.iter().any(|p| p == "network") {
                        register_fetch(&ctx, tx.clone(), rt_handle.clone())?;
                        register_websocket_with_config(&ctx, tx, ws, rt_handle, websocket_config)?;
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
                    register_filesystem(&ctx, &perms)?;
                    Ok(crypto)
                },
            )
            .await?
        };
        Ok(Engine {
            rt,
            ctx,
            crypto,
            _action_tx: action_tx,
            action_rx: tokio::sync::Mutex::new(action_rx),
            _timer_handles: timer_handles,
            _ws_senders: ws_senders,
            terminals,
            _native_runtime: native_runtime,
            native_completion_rx: tokio::sync::Mutex::new(native_completion_rx),
        })
    }
}

pub(super) struct ShutdownOnExit<'a>(pub(super) &'a Engine);
impl Drop for ShutdownOnExit<'_> {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}
