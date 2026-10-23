# Native modules (embedding API)

Applications embedding `tela-engine` can expose app-owned async Rust capabilities without adding application dependencies to the engine:

```rust
use tela_engine::{Engine, NativeModule};

let engine = Engine::new_with_native_modules(
    "my-app",
    &permissions,
    vec![NativeModule::new("remote", "remote", |operation, params, cancel| async move {
        // Check `cancel` during long-running work. Return only data intended for JS.
        Ok(serde_json::json!({ "operation": operation, "result": params }))
    })],
).await?;
```

The JavaScript surface is `Tela.native.has(name)` and
`Tela.native.call(name, operation, params)`, where `call` returns a Promise.
A module is registered by name but can only execute when its declared permission
is present in the engine permissions. Unknown modules, denied permissions, invalid
parameters, and overload reject the Promise before the handler runs.

Handlers receive `(String, serde_json::Value, tela_engine::CancellationToken)`
and return `Result<serde_json::Value, String>`. The token is cancelled by `Engine::shutdown()` (also called when `run()` exits)
or when the engine is dropped. The engine also drops pending handler futures on cancellation even if they ignore the token, and suppresses queued completions after shutdown. Handlers must own/cancel any background tasks they create themselves; detached work is not implicitly managed. The engine never calls QuickJS from a handler task. Return
only data intended for JS; keep credential/private-key material native.

Native calls have at most 8 total queued/running/completed-undrained requests,
64 KiB serialized parameters, operation names of 1–128 bytes, and 8 MiB serialized successful responses. The
pending limit is released only after completion is delivered on the engine's
QuickJS thread, so a paused UI cannot accumulate completed responses.
