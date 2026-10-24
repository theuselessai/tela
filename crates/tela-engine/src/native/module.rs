use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use rquickjs::Ctx;
use serde_json::Value;
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const MAX_PENDING: usize = 8;
const MAX_PARAMS_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

pub type NativeModuleFuture = Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'static>>;
pub type NativeModuleHandler =
    Arc<dyn Fn(String, Value, CancellationToken) -> NativeModuleFuture + Send + Sync + 'static>;

/// An application-provided, permission-gated async native API.
pub struct NativeModule {
    pub name: String,
    pub permission: String,
    pub handler: NativeModuleHandler,
}

impl NativeModule {
    pub fn new<F, Fut>(name: impl Into<String>, permission: impl Into<String>, handler: F) -> Self
    where
        F: Fn(String, Value, CancellationToken) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Value, String>> + Send + 'static,
    {
        Self {
            name: name.into(),
            permission: permission.into(),
            handler: Arc::new(move |operation, params, cancellation| {
                Box::pin(handler(operation, params, cancellation))
            }),
        }
    }
}

struct RegisteredModule {
    permission: String,
    handler: NativeModuleHandler,
}

struct NativeRequest {
    id: u64,
    handler: NativeModuleHandler,
    operation: String,
    params: Value,
    permit: OwnedSemaphorePermit,
}

pub(crate) struct NativeCompletion {
    pub(crate) id: u64,
    pub(crate) result: Result<String, String>,
    _permit: OwnedSemaphorePermit,
}

pub(crate) struct NativeModuleRuntime {
    modules: HashMap<String, RegisteredModule>,
    request_tx: mpsc::Sender<NativeRequest>,
    cancellation: CancellationToken,
    pending: Arc<Semaphore>,
    worker: JoinHandle<()>,
}

impl NativeModuleRuntime {
    pub fn new(
        modules: Vec<NativeModule>,
    ) -> anyhow::Result<(Arc<Self>, mpsc::Receiver<NativeCompletion>)> {
        let mut registered = HashMap::new();
        for module in modules {
            if module.name.is_empty() || module.permission.is_empty() {
                anyhow::bail!("native module name and permission must not be empty");
            }
            if registered
                .insert(
                    module.name.clone(),
                    RegisteredModule {
                        permission: module.permission,
                        handler: module.handler,
                    },
                )
                .is_some()
            {
                anyhow::bail!("duplicate native module: {}", module.name);
            }
        }

        let (request_tx, mut request_rx) = mpsc::channel::<NativeRequest>(MAX_PENDING);
        let (completion_tx, completion_rx) = mpsc::channel::<NativeCompletion>(MAX_PENDING);
        let cancellation = CancellationToken::new();
        let worker_cancellation = cancellation.clone();
        let worker = tokio::spawn(async move {
            loop {
                let request = tokio::select! {
                    biased;
                    _ = worker_cancellation.cancelled() => break,
                    request = request_rx.recv() => match request { Some(request) => request, None => break },
                };
                let completion_tx = completion_tx.clone();
                let cancellation = worker_cancellation.clone();
                tokio::spawn(async move {
                    if cancellation.is_cancelled() {
                        return;
                    }
                    let work =
                        (request.handler)(request.operation, request.params, cancellation.clone());
                    let result = tokio::select! {
                        biased;
                        _ = cancellation.cancelled() => return,
                        result = work => result,
                    };
                    if cancellation.is_cancelled() {
                        return;
                    }
                    let result = match result {
                        Ok(value) => match serde_json::to_string(&value) {
                            Ok(json) if json.len() <= MAX_RESPONSE_BYTES => Ok(json),
                            Ok(_) => Err("native module response too large".to_string()),
                            Err(_) => Err("native module returned invalid JSON".to_string()),
                        },
                        Err(error) if error.len() <= MAX_PARAMS_BYTES => Err(error),
                        Err(_) => Err("native module failed".to_string()),
                    };
                    let completion = NativeCompletion {
                        id: request.id,
                        result,
                        _permit: request.permit,
                    };
                    tokio::select! {
                        biased;
                        _ = cancellation.cancelled() => {},
                        _ = completion_tx.send(completion) => {},
                    }
                });
            }
        });
        Ok((
            Arc::new(Self {
                modules: registered,
                request_tx,
                cancellation,
                pending: Arc::new(Semaphore::new(MAX_PENDING)),
                worker,
            }),
            completion_rx,
        ))
    }

    pub(crate) fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    fn call(
        &self,
        id: u64,
        name: String,
        operation: String,
        params_json: String,
        permissions: &[String],
    ) -> Option<String> {
        if self.cancellation.is_cancelled() {
            return Some("native module shutting down".to_string());
        }
        let Some(module) = self.modules.get(&name) else {
            return Some("native module not found".to_string());
        };
        if !permissions
            .iter()
            .any(|permission| permission == &module.permission)
        {
            return Some("native permission denied".to_string());
        }
        if operation.is_empty() || operation.len() > 128 {
            return Some("native module operation invalid".to_string());
        }
        if params_json.len() > MAX_PARAMS_BYTES {
            return Some("native module params too large".to_string());
        }
        let params = match serde_json::from_str(&params_json) {
            Ok(params) => params,
            Err(_) => return Some("native module params invalid".to_string()),
        };
        let permit = match self.pending.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => return Some("native module overloaded".to_string()),
        };
        let request = NativeRequest {
            id,
            handler: module.handler.clone(),
            operation,
            params,
            permit,
        };
        self.request_tx
            .try_send(request)
            .err()
            .map(|_| "native module overloaded".to_string())
    }
}

impl Drop for NativeModuleRuntime {
    fn drop(&mut self) {
        self.cancellation.cancel();
        self.worker.abort();
    }
}

const NATIVE_BOOTSTRAP: &str = r#"
globalThis.__tela_native_resolvers__ = {};
globalThis.__tela_next_native_id__ = 1;
globalThis.__tela_resolve_native__ = function(id, resultJson, error) {
    var entry = globalThis.__tela_native_resolvers__[id];
    if (!entry) return;
    delete globalThis.__tela_native_resolvers__[id];
    if (error) { entry.reject(new Error(error)); return; }
    try { entry.resolve(JSON.parse(resultJson)); }
    catch (e) { entry.reject(e); }
};
globalThis.Tela.native = {
    has: function(name) { return __tela_native_has__(String(name)); },
    call: function(name, operation, params) {
        var id = globalThis.__tela_next_native_id__++;
        var json;
        try { json = JSON.stringify(params === undefined ? null : params); }
        catch (_) { return Promise.reject(new Error("native module params invalid")); }
        if (json === undefined) return Promise.reject(new Error("native module params invalid"));
        var error = __tela_native_call__(id, String(name), String(operation), json);
        if (error) return Promise.reject(new Error(error));
        return new Promise(function(resolve, reject) {
            globalThis.__tela_native_resolvers__[id] = { resolve: resolve, reject: reject };
        });
    },
};
"#;

pub(crate) fn register_native_modules(
    ctx: &Ctx<'_>,
    runtime: Arc<NativeModuleRuntime>,
    permissions: Vec<String>,
) -> anyhow::Result<()> {
    let has_runtime = runtime.clone();
    ctx.globals().set(
        "__tela_native_has__",
        rquickjs::Function::new(ctx.clone(), move |name: String| {
            has_runtime.modules.contains_key(&name)
        })?,
    )?;
    ctx.globals().set(
        "__tela_native_call__",
        rquickjs::Function::new(
            ctx.clone(),
            move |id: u64, name: String, operation: String, params_json: String| {
                runtime.call(id, name, operation, params_json, &permissions)
            },
        )?,
    )?;
    ctx.eval::<(), _>(NATIVE_BOOTSTRAP)
        .map_err(|e| anyhow::anyhow!("failed to register native module bootstrap: {e}"))?;
    Ok(())
}

#[cfg(test)]
#[path = "module_tests.rs"]
mod tests;
