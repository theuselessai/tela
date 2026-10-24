use super::{bytes, fail, integer, output, CryptoRuntime};
use argon2::{Algorithm, Argon2, Block, Params, Version};
use rquickjs::{function::Rest, Ctx, Function, Object, Result, Value};
use std::sync::{atomic::Ordering, Arc};
use tokio::sync::Semaphore;
use zeroize::{Zeroize, Zeroizing};

// No queue. This permit moves into the blocking closure, not its async waiter.
static ADMISSION: Semaphore = Semaphore::const_new(1);
type Outcome = std::result::Result<Zeroizing<Vec<u8>>, &'static str>;

#[derive(Default)]
pub(super) struct Jobs {
    next: u32,
    current: Option<Job>,
}
struct Job {
    id: u32,
    outcome: Option<Outcome>,
}
impl Jobs {
    pub(super) fn clear(&mut self) {
        self.current.take();
    }
}

// Explicit scratch ownership also wipes on unwind without requiring optional
// argon2 provider features. Provider-internal stack temporaries are not promised.
struct Memory(Vec<Block>);
impl Drop for Memory {
    fn drop(&mut self) {
        for block in &mut self.0 {
            block.as_mut().zeroize();
        }
    }
}

fn compute(password: Zeroizing<Vec<u8>>, salt: Zeroizing<Vec<u8>>, params: Params) -> Outcome {
    let mut out = Zeroizing::new(vec![0u8; params.output_len().unwrap()]);
    let count = params.block_count();
    let mut blocks = Vec::new();
    blocks
        .try_reserve_exact(count)
        .map_err(|_| "crypto Argon2 memory unavailable")?;
    blocks.resize(count, Block::default());
    let mut memory = Memory(blocks);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into_with_memory(&password, &salt, &mut out, &mut memory.0)
        .map_err(|_| "crypto Argon2 failed")?;
    Ok(out)
}

/// All captured data is owned Rust memory, never a JS value. `work` and its
/// scratch/password/output cleanup finish before admission can be reacquired.
fn admit(
    state: &Arc<CryptoRuntime>,
    work: impl FnOnce() -> Outcome + Send + 'static,
) -> std::result::Result<u32, &'static str> {
    let handle = tokio::runtime::Handle::try_current().map_err(|_| "crypto worker unavailable")?;
    let mut jobs = state.jobs.lock().unwrap();
    if !state.alive.load(Ordering::Acquire) {
        return Err("crypto context unavailable");
    }
    if jobs.current.is_some() {
        return Err("crypto Argon2 busy");
    }
    let permit = ADMISSION.try_acquire().map_err(|_| "crypto Argon2 busy")?;
    let id = jobs
        .next
        .checked_add(1)
        .ok_or("crypto job limit exceeded")?;
    jobs.next = id;
    jobs.current = Some(Job { id, outcome: None });
    let weak = Arc::downgrade(state);
    handle.spawn_blocking(move || {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work))
            .unwrap_or(Err("crypto Argon2 failed"));
        // Cancellation only removes the delivery slot; it never owns the permit.
        // On cancellation, wipe output before releasing admission as well.
        if let Some(state) = weak.upgrade() {
            let mut jobs = state.jobs.lock().unwrap();
            if state.alive.load(Ordering::Acquire) {
                if let Some(job) = jobs.current.as_mut().filter(|job| job.id == id) {
                    job.outcome = Some(outcome);
                    drop(permit);
                    state.ready.notify_one();
                    return;
                }
            }
        }
        drop(outcome);
        drop(permit);
    });
    Ok(id)
}

fn cancel(state: &CryptoRuntime, id: u32) {
    let mut jobs = state.jobs.lock().unwrap();
    if jobs.current.as_ref().is_some_and(|job| job.id == id) {
        jobs.current.take();
    }
}

fn start(ctx: &Ctx<'_>, state: &Arc<CryptoRuntime>, args: &[Value<'_>]) -> Result<u32> {
    state.check(ctx)?;
    if args.len() != 3 {
        return Err(fail(ctx, "crypto invalid argument count"));
    }
    let password = bytes(ctx, &args[0], 4096)?;
    let salt = bytes(ctx, &args[1], 64)?;
    if salt.len() < 8 {
        return Err(fail(ctx, "crypto invalid salt length"));
    }
    let options = args[2]
        .as_object()
        .ok_or_else(|| fail(ctx, "crypto invalid Argon2 options"))?;
    let param = |name: &str, min, max| -> Result<usize> {
        let v: Value = options.get(name)?;
        integer(ctx, &v, min, max)
    };
    param("version", 19, 19)?;
    let memory = param("memoryKiB", 8, 65536)?;
    let passes = param("passes", 1, 3)?;
    param("parallelism", 1, 1)?;
    let output_len = param("outputLength", 4, 1024)?;
    // Pass the exact requested m_cost to the provider. Argon2's standard block
    // layout may use fewer blocks, but its hash still commits to the exact cost.
    let params = Params::new(memory as u32, passes as u32, 1, Some(output_len))
        .map_err(|_| fail(ctx, "crypto unsupported Argon2 parameters"))?;
    admit(state, move || compute(password, salt, params)).map_err(|e| fail(ctx, e))
}

const BOOTSTRAP: &str = r#"
(function(start, cancel) {
    let pending = null;
    return {
        argon2id: function(password, salt, options) {
            // Validation and admission fail synchronously, before a job exists.
            const id = start(password, salt, options);
            let resolve, reject;
            const result = new Promise((yes, no) => { resolve = yes; reject = no; });
            const entry = {id, resolve, reject};
            pending = entry;
            return {result, cancel: function() {
                if (pending !== entry) return;
                cancel(id);
                pending = null;
                reject(new Error('crypto Argon2 cancelled'));
            }};
        },
        deliver: function(id, value, error) {
            if (!pending || pending.id !== id) return;
            const entry = pending;
            pending = null;
            if (error) entry.reject(new Error(error));
            else entry.resolve(value);
        }
    };
})
"#;

pub(super) fn register<'js>(
    ctx: &Ctx<'js>,
    state: &Arc<CryptoRuntime>,
    crypto: &Object<'js>,
) -> Result<()> {
    let start_state = state.clone();
    let start = Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Rest<Value<'js>>| {
        start(&ctx, &start_state, &args.0)
    })?;
    let cancel_state = state.clone();
    let cancel = Function::new(
        ctx.clone(),
        move |ctx: Ctx<'js>, id: Value<'js>| -> Result<()> {
            cancel_state.check(&ctx)?;
            let id = integer(&ctx, &id, 1, u32::MAX as usize)?;
            cancel(&cancel_state, id as u32);
            Ok(())
        },
    )?;
    let factory: Function = ctx.eval(BOOTSTRAP)?;
    let api: Object = factory.call((start, cancel))?;
    crypto.set("argon2id", api.get::<_, Function>("argon2id")?)?;
    ctx.globals().set(
        "__tela_crypto_complete__",
        api.get::<_, Function>("deliver")?,
    )?;
    Ok(())
}

pub(super) fn deliver(ctx: &Ctx<'_>, state: &Arc<CryptoRuntime>) -> Result<bool> {
    let completion = {
        let mut jobs = state.jobs.lock().unwrap();
        if jobs.current.as_ref().is_some_and(|j| j.outcome.is_some()) {
            jobs.current.take()
        } else {
            None
        }
    };
    let Some(mut job) = completion else {
        return Ok(false);
    };
    if !state.alive.load(Ordering::Acquire) {
        return Ok(false);
    }
    let (value, error) = match job.outcome.take().unwrap() {
        Ok(bytes) => (output(ctx, &bytes)?, None),
        Err(error) => (Value::new_undefined(ctx.clone()), Some(error)),
    };
    let resolver: Function = ctx.globals().get("__tela_crypto_complete__")?;
    resolver.call::<_, ()>((job.id, value, error))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    // All tests acquiring the process-wide production semaphore share this lock.
    pub(crate) static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancellation_keeps_permit_until_blocking_cleanup() {
        let _serial = SERIAL.lock().await;
        let rt = rquickjs::Runtime::new().unwrap();
        let context = rquickjs::Context::full(&rt).unwrap();
        let state = context.with(|ctx| {
            ctx.eval::<(), _>("globalThis.Tela = {};").unwrap();
            super::super::register_crypto(&ctx).unwrap()
        });
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let id = admit(&state, move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(Zeroizing::new(vec![1; 80]))
        })
        .unwrap();
        entered_rx.recv().unwrap();
        cancel(&state, id);
        assert!(state.jobs.lock().unwrap().current.is_none());
        assert!(admit(&state, || Ok(Zeroizing::new(vec![]))).is_err());
        // A second context cannot bypass the global permit either.
        let other = rquickjs::Context::full(&rt).unwrap();
        let other_state = other.with(|ctx| {
            ctx.eval::<(), _>("globalThis.Tela = {};").unwrap();
            super::super::register_crypto(&ctx).unwrap()
        });
        assert!(admit(&other_state, || Ok(Zeroizing::new(vec![]))).is_err());
        state.shutdown();
        release_tx.send(()).unwrap();
        // Await actual release, not the cancellation request or worker wakeup.
        let permit = tokio::time::timeout(std::time::Duration::from_secs(5), ADMISSION.acquire())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
        assert!(state.jobs.lock().unwrap().current.is_none());
        context.with(|ctx| assert!(!super::super::deliver_crypto(&ctx, &state).unwrap()));
        other_state.shutdown();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn teardown_never_delivers_to_dropped_context() {
        let _serial = SERIAL.lock().await;
        let rt = rquickjs::Runtime::new().unwrap();
        let context = rquickjs::Context::full(&rt).unwrap();
        let state = context.with(|ctx| {
            ctx.eval::<(), _>("globalThis.Tela = {};").unwrap();
            super::super::register_crypto(&ctx).unwrap()
        });
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        admit(&state, move || {
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            Ok(Zeroizing::new(vec![42; 80]))
        })
        .unwrap();
        entered_rx.recv().unwrap();
        state.shutdown();
        drop(context);
        drop(rt);
        assert!(ADMISSION.try_acquire().is_err());
        release_tx.send(()).unwrap();
        super::wait_idle().await;
        assert!(state.jobs.lock().unwrap().current.is_none());
    }

    #[test]
    fn exact_large_profile_matches_provider() {
        let password = Zeroizing::new(b"synthetic password".to_vec());
        let salt = Zeroizing::new(b"synthetic salt!!".to_vec());
        let params = Params::new(65536, 3, 1, Some(80)).unwrap();
        let actual = compute(password.clone(), salt.clone(), params.clone()).unwrap();
        let mut expected = [0u8; 80];
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(&password, &salt, &mut expected)
            .unwrap();
        assert_eq!(&*actual, &expected);
    }
}

#[cfg(test)]
pub(crate) use tests::SERIAL;

#[cfg(test)]
pub(super) async fn wait_idle() {
    let permit = tokio::time::timeout(std::time::Duration::from_secs(10), ADMISSION.acquire())
        .await
        .unwrap()
        .unwrap();
    drop(permit);
}
