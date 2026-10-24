//! Small, typed RustCrypto bindings; no protocol policy or JSON byte bridge.
mod argon;
mod handles;
mod ops;
#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use argon::SERIAL as ARGON_TEST_SERIAL;

use rquickjs::{function::Rest, Ctx, Exception, Function, Object, Result, TypedArray, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, Weak,
};
use tokio::sync::Notify;
use zeroize::Zeroizing;

pub(crate) const MAX_BYTES: usize = 1024 * 1024;
const MAX_HANDLES: usize = 256;

/// One instance per registered context. Contains no JS values and may be shut down
/// without entering QuickJS. Engine must call shutdown before dropping its context.
pub(crate) struct CryptoRuntime {
    context: usize,
    alive: AtomicBool,
    handles: Mutex<Vec<Weak<handles::Slot>>>,
    jobs: Mutex<argon::Jobs>,
    ready: Notify,
}

impl CryptoRuntime {
    fn check(&self, ctx: &Ctx<'_>) -> Result<()> {
        if !self.alive.load(Ordering::Acquire) || self.context != ctx.as_raw().as_ptr() as usize {
            return Err(fail(ctx, "crypto context unavailable"));
        }
        Ok(())
    }

    /// Cancels delivery, drops queued output, and wipes all owned native handles.
    /// This does not interrupt an active Argon2 computation or release its permit.
    pub(crate) fn shutdown(&self) {
        self.alive.store(false, Ordering::Release);
        self.jobs.lock().unwrap().clear();
        for slot in self
            .handles
            .lock()
            .unwrap()
            .drain(..)
            .filter_map(|s| s.upgrade())
        {
            slot.dispose();
        }
    }

    /// Select this alongside terminal/native events. A stored notification avoids
    /// lost wakeups when computation finishes before Engine begins selecting.
    pub(crate) async fn notified(&self) {
        self.ready.notified().await;
    }
}

impl Drop for CryptoRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(super) fn fail(ctx: &Ctx<'_>, message: &str) -> rquickjs::Error {
    Exception::throw_type(ctx, message)
}

pub(super) fn bytes(ctx: &Ctx<'_>, value: &Value<'_>, max: usize) -> Result<Zeroizing<Vec<u8>>> {
    let array = TypedArray::<u8>::from_value(value.clone())
        .map_err(|_| fail(ctx, "crypto requires Uint8Array"))?;
    let data = array
        .as_bytes()
        .ok_or_else(|| fail(ctx, "crypto detached byte view"))?;
    if data.len() > max {
        return Err(fail(ctx, "crypto byte limit exceeded"));
    }
    Ok(Zeroizing::new(data.to_vec()))
}

pub(super) fn fixed<const N: usize>(
    ctx: &Ctx<'_>,
    value: &Value<'_>,
) -> Result<Zeroizing<[u8; N]>> {
    let data = bytes(ctx, value, N)?;
    let array = data
        .as_slice()
        .try_into()
        .map_err(|_| fail(ctx, "crypto invalid byte length"))?;
    Ok(Zeroizing::new(array))
}

pub(super) fn integer(ctx: &Ctx<'_>, value: &Value<'_>, min: usize, max: usize) -> Result<usize> {
    let n = value
        .as_number()
        .ok_or_else(|| fail(ctx, "crypto requires integer"))?;
    if !n.is_finite() || n.fract() != 0.0 || n < min as f64 || n > max as f64 {
        return Err(fail(ctx, "crypto integer out of range"));
    }
    Ok(n as usize)
}

pub(super) fn output<'js>(ctx: &Ctx<'js>, data: &[u8]) -> Result<Value<'js>> {
    if data.len() > MAX_BYTES {
        return Err(fail(ctx, "crypto byte limit exceeded"));
    }
    Ok(TypedArray::<u8>::new_copy(ctx.clone(), data)?.into_value())
}

fn functions<'js>(
    ctx: &Ctx<'js>,
    state: &Arc<CryptoRuntime>,
    target: &Object<'js>,
    names: &[&'static str],
) -> Result<()> {
    for &name in names {
        let state = state.clone();
        target.set(
            name,
            Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
                state.check(&ctx)?;
                ops::dispatch(&ctx, &state, name, &args.0)
            })?,
        )?;
    }
    Ok(())
}

/// Register only when the manifest grants `crypto`, after creating global Tela.
/// Register once per context; retain the returned runtime in Engine.
pub(crate) fn register_crypto(ctx: &Ctx<'_>) -> Result<Arc<CryptoRuntime>> {
    let tela: Object = ctx.globals().get("Tela")?;
    if tela.contains_key("crypto")? {
        return Err(fail(ctx, "crypto already registered"));
    }
    let state = Arc::new(CryptoRuntime {
        context: ctx.as_raw().as_ptr() as usize,
        alive: AtomicBool::new(true),
        handles: Mutex::new(Vec::new()),
        jobs: Mutex::new(argon::Jobs::default()),
        ready: Notify::new(),
    });
    let crypto = Object::new(ctx.clone())?;
    functions(
        ctx,
        &state,
        &crypto,
        &[
            "randomBytes",
            "sha256",
            "hmacSha256",
            "hkdfSha256",
            "timingSafeEqual",
            "ed25519Verify",
        ],
    )?;
    for (name, methods) in [
        (
            "p256",
            &[
                "scalarRandomNonzero",
                "scalarImport32",
                "scalarReduceBE",
                "scalarExport32",
                "pointImport65",
                "pointExport65",
                "baseMul",
                "mul",
                "add",
                "sub",
                "isIdentity",
            ][..],
        ),
        (
            "x25519",
            &["generate", "importPrivate32", "publicKey", "derive"][..],
        ),
        ("chacha20poly1305", &["importKey32", "seal", "open"][..]),
    ] {
        let group = Object::new(ctx.clone())?;
        functions(ctx, &state, &group, methods)?;
        crypto.set(name, group)?;
    }
    argon::register(ctx, &state, &crypto)?;
    tela.set("crypto", crypto)?;
    Ok(state)
}

/// Deliver at most one completion inside the owning `ctx.with`. Returns whether
/// a promise was settled. Afterwards Engine must execute JS jobs and dispatches.
/// Safe after shutdown: returns false without accessing any JS value.
pub(crate) fn deliver_crypto(ctx: &Ctx<'_>, state: &Arc<CryptoRuntime>) -> Result<bool> {
    if !state.alive.load(Ordering::Acquire) {
        return Ok(false);
    }
    state.check(ctx)?;
    argon::deliver(ctx, state)
}
