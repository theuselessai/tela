use super::*;
use rquickjs::{Context, Runtime};

const HELPERS: &str = r#"
const c = Tela.crypto, p = c.p256, x = c.x25519, aead = c.chacha20poly1305;
function assert(v) { if (!v) throw Error('assertion failed'); }
function hex(bytes) { return Array.from(bytes, b => b.toString(16).padStart(2, '0')).join(''); }
function unhex(s) { return new Uint8Array(s.match(/../g).map(b => parseInt(b, 16))); }
function throws(f) { let threw = false; try { f(); } catch (_) { threw = true; } assert(threw); }
function same(a, b) { assert(c.timingSafeEqual(a, b)); }
function b(...values) { return new Uint8Array(values); }
"#;

fn setup(ctx: &Ctx<'_>) -> Arc<CryptoRuntime> {
    ctx.eval::<(), _>("globalThis.Tela = {};").unwrap();
    let state = register_crypto(ctx).unwrap();
    ctx.eval::<(), _>(HELPERS).unwrap();
    state
}
fn eval(ctx: &Ctx<'_>, script: &str) {
    if let Err(e) = ctx.eval::<(), _>(script) {
        panic!("JS test failed: {e}: {:?}", ctx.catch());
    }
}
fn check(script: &str) {
    let rt = Runtime::new().unwrap();
    let context = Context::full(&rt).unwrap();
    context.with(|ctx| {
        let state = setup(&ctx);
        eval(&ctx, script);
        state.shutdown();
    });
}

#[test]
fn hash_hmac_hkdf_vectors_and_byte_views() {
    check(
        r#"
        const backing = b(0, 97, 98, 99, 255), view = backing.subarray(1, 4);
        Object.defineProperty(view, 'length', {value: 999999999});
        assert(hex(c.sha256(view)) === 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
        assert(hex(c.hmacSha256(new Uint8Array(20).fill(11), unhex('4869205468657265'))) ===
            'b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7');
        const hk = c.hkdfSha256(new Uint8Array(22).fill(11), unhex('000102030405060708090a0b0c'),
            unhex('f0f1f2f3f4f5f6f7f8f9'), 42);
        assert(hex(hk) === '3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865');
        assert(c.hkdfSha256(b(), null, b(), 8160).length === 8160);
        assert(c.hkdfSha256(b(), null, b(), 0).length === 0);
        same(c.hkdfSha256(b(1), null, b(), 32), c.hkdfSha256(b(1), b(), b(), 32));
        assert(c.timingSafeEqual(b(), b()));
        assert(!c.timingSafeEqual(b(1), b(1, 0)));
        assert(!c.timingSafeEqual(b(1), b(2)));
        const out = c.sha256(view); out[0] = 0;
        assert(c.sha256(view)[0] === 186 && backing[1] === 97);
        assert(c.randomBytes(0) instanceof Uint8Array);
        assert(c.randomBytes(1048576).length === 1048576);
        for (const n of [-1, 1.5, NaN, Infinity, '2', 1048577]) throws(() => c.randomBytes(n));
        for (const input of [[1], 'a', {}, new Int8Array(1), new Uint8ClampedArray(1),
                new Uint16Array(1), new ArrayBuffer(1), new DataView(new ArrayBuffer(1))]) {
            throws(() => c.sha256(input));
        }
        throws(() => c.sha256(new Uint8Array(1048577)));
        throws(() => c.hkdfSha256(b(), undefined, b(), 1));
        throws(() => c.hkdfSha256(b(), null, b(), 8161));
        throws(() => c.sha256());
    "#,
    );
}

#[test]
fn detached_views_fail_without_panicking() {
    let rt = Runtime::new().unwrap();
    let context = Context::full(&rt).unwrap();
    context.with(|ctx| {
        let state = setup(&ctx);
        let view: TypedArray<u8> = ctx.eval("new Uint8Array([1,2,3])").unwrap();
        view.arraybuffer().unwrap().detach();
        ctx.globals().set("detached", view).unwrap();
        eval(
            &ctx,
            "throws(() => c.sha256(detached)); throws(() => p.scalarReduceBE(detached));",
        );
        state.shutdown();
    });
}

#[test]
fn canonical_scalars_points_and_identity_arithmetic() {
    check(
        r#"
        const order = unhex('ffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551');
        throws(() => p.scalarImport32(order));
        throws(() => p.scalarImport32(new Uint8Array(32).fill(255)));
        throws(() => p.scalarImport32(b(1)));
        throws(() => p.scalarReduceBE(new Uint8Array(65)));
        const zero = p.scalarReduceBE(order), one = p.scalarReduceBE(b(1));
        same(p.scalarExport32(zero), new Uint8Array(32));
        same(p.scalarExport32(p.scalarReduceBE(b())), new Uint8Array(32));
        const orderPlusOne = order.slice(); orderPlusOne[31]++;
        same(p.scalarExport32(p.scalarReduceBE(orderPlusOne)), p.scalarExport32(one));
        const large = new Uint8Array(64); large.set(order, 0);
        same(p.scalarExport32(p.scalarReduceBE(large)), new Uint8Array(32));
        const G = p.baseMul(one), identity = p.baseMul(zero);
        assert(hex(p.pointExport65(G)) === '046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c2964fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5');
        const raw = p.pointExport65(G), padded = new Uint8Array(70); padded.set(raw, 3);
        same(p.pointExport65(p.pointImport65(padded.subarray(3,68))), raw);
        same(p.pointExport65(p.add(G, identity)), raw);
        same(p.pointExport65(p.mul(G, one)), raw);
        assert(p.isIdentity(identity));
        assert(p.isIdentity(p.sub(G, G)));
        assert(p.isIdentity(p.mul(G, zero)));
        same(p.pointExport65(p.add(G, G)), p.pointExport65(p.baseMul(p.scalarReduceBE(b(2)))));
        throws(() => p.pointExport65(identity));
        throws(() => p.pointImport65(b(0)));
        throws(() => p.pointImport65(raw.subarray(0,33)));
        const bad = raw.slice(); bad[0] = 6; throws(() => p.pointImport65(bad));
        const offCurve = new Uint8Array(65); offCurve[0] = 4; throws(() => p.pointImport65(offCurve));
        const noncanonical = raw.slice(); noncanonical.fill(255,1,33); throws(() => p.pointImport65(noncanonical));
        for (let i=0;i<8;i++) {
            const s = p.scalarRandomNonzero();
            assert(!p.isIdentity(p.baseMul(s))); s.dispose();
        }
    "#,
    );
}

#[test]
fn x25519_rfc7748_zero_rejection_and_aead_exact_bytes() {
    check(
        r#"
        const alice = x.importPrivate32(unhex('77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a'));
        const bob = unhex('de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f');
        assert(hex(x.publicKey(alice)) === '8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a');
        assert(hex(x.derive(alice, bob)) === '4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742');
        throws(() => x.derive(alice, new Uint8Array(32)));
        const lowOrder = new Uint8Array(32); lowOrder[0] = 1; throws(() => x.derive(alice, lowOrder));
        throws(() => x.derive(alice, b(1)));
        throws(() => x.importPrivate32(new Uint8Array(31)));
        const generated = x.generate(); assert(x.publicKey(generated).length === 32);
        const key = aead.importKey32(new Uint8Array(32)), nonce = new Uint8Array(12);
        const sealed = aead.seal(key, nonce, b(), b());
        assert(hex(sealed) === '4eb972c9a8fb3a1b382bb4d36f5ffad1');
        same(aead.open(key, nonce, b(), sealed), b());
        const ct = aead.seal(key, nonce, b(1), b(2,3,4));
        assert(ct.length === 19); same(aead.open(key, nonce, b(1), ct), b(2,3,4));
        throws(() => aead.open(key, nonce, b(2), ct));
        ct[ct.length-1] ^= 1; throws(() => aead.open(key, nonce, b(1), ct));
        throws(() => aead.open(key, nonce, b(), new Uint8Array(15)));
        throws(() => aead.seal(key, b(), b(), b()));
        throws(() => aead.importKey32(b()));
        throws(() => aead.seal(key, nonce, b(), new Uint8Array(1048576-15)));
        const max = aead.seal(key, nonce, b(), new Uint8Array(1048576-16));
        assert(max.length === 1048576 && aead.open(key, nonce, b(), max).length === 1048576-16);
        throws(() => x.publicKey(key)); throws(() => aead.seal(alice, nonce, b(), b()));
    "#,
    );
}

#[test]
fn ed25519_dalek_verifier_policy() {
    check(
        r#"
        const pk = unhex('d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a');
        const sig = unhex('e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b');
        assert(c.ed25519Verify(pk, b(), sig));
        assert(!c.ed25519Verify(pk, b(1), sig));
        sig[0] ^= 1; assert(!c.ed25519Verify(pk, b(), sig));
        throws(() => c.ed25519Verify(b(), b(), sig));
        throws(() => c.ed25519Verify(pk, b(), sig.subarray(1)));
        // Dalek's non-strict policy accepts this small-order synthetic equation.
        const identity = new Uint8Array(32); identity[0] = 1;
        const weakSig = new Uint8Array(64); weakSig[0] = 1;
        assert(c.ed25519Verify(identity, b(), weakSig));
    "#,
    );
}

#[test]
fn disposal_bound_gc_wrong_kind_and_shutdown() {
    let rt = Runtime::new().unwrap();
    let context = Context::full(&rt).unwrap();
    let state = context.with(|ctx| {
        let state = setup(&ctx);
        eval(
            &ctx,
            r#"
            globalThis.live = Array.from({length:256}, () => p.scalarReduceBE(b(1)));
            throws(() => p.scalarReduceBE(b(1)));
            live[0].dispose(); live[0].dispose();
            throws(() => p.scalarExport32(live[0]));
            live[0] = p.scalarReduceBE(b(1));
            throws(() => p.pointExport65(live[0]));
            throws(() => p.scalarExport32({}));
            throws(() => p.scalarExport32(Object.create(Object.getPrototypeOf(live[0]))));
            for (const value of live) value.dispose();
            globalThis.survivor = p.scalarReduceBE(b(7));
            live = null;
            for (let i = 0; i < 500; i++) { p.scalarReduceBE(b(1)); }
            const point = p.baseMul(survivor), privateKey = x.generate(), key = aead.importKey32(new Uint8Array(32));
            point.dispose(); privateKey.dispose(); key.dispose();
            point.dispose(); privateKey.dispose(); key.dispose();
            throws(() => p.isIdentity(point)); throws(() => x.publicKey(privateKey));
            throws(() => aead.seal(key, new Uint8Array(12), b(), b()));
            // Include cyclic JS wrappers so the test exercises QuickJS GC, not
            // merely immediate reference-count destruction.
            globalThis.cycles = Array.from({length: 200}, () => {
                const s = p.scalarReduceBE(b(1)); s.self = s; return s;
            });
            cycles = null;
        "#,
        );
        state
    });
    rt.run_gc();
    assert_eq!(
        state
            .handles
            .lock()
            .unwrap()
            .iter()
            .filter(|s| s.strong_count() > 0)
            .count(),
        1
    );
    state.shutdown();
    context.with(|ctx| eval(&ctx, "throws(() => p.scalarExport32(survivor)); throws(() => c.sha256(b())); survivor.dispose();"));
    assert!(state.handles.lock().unwrap().is_empty());
}

#[test]
fn rejects_cross_context_handles_and_delivery() {
    let rt = Runtime::new().unwrap();
    let first = Context::full(&rt).unwrap();
    let second = Context::full(&rt).unwrap();
    let (s1, saved) = first.with(|ctx1| {
        let s1 = setup(&ctx1);
        let value: Value = ctx1.eval("p.scalarReduceBE(b(1))").unwrap();
        (s1, rquickjs::Persistent::save(&ctx1, value))
    });
    second.with(|ctx2| {
        let s2 = setup(&ctx2);
        ctx2.globals()
            .set("foreign", saved.restore(&ctx2).unwrap())
            .unwrap();
        eval(&ctx2, "throws(() => p.scalarExport32(foreign));");
        assert!(deliver_crypto(&ctx2, &s1).is_err());
        ctx2.catch();
        s2.shutdown();
    });
    s1.shutdown();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn argon_quickjs_delivery_validation_and_cancel() {
    let _serial = super::argon::SERIAL.lock().await;
    let rt = Runtime::new().unwrap();
    let context = Context::full(&rt).unwrap();
    let state = context.with(|ctx| {
        let state = setup(&ctx);
        eval(&ctx, r#"
            globalThis.opts = {version:19,memoryKiB:9,passes:1,parallelism:1,outputLength:32};
            for (const [key, value] of [['version',16],['memoryKiB',7],['memoryKiB',65537],
                ['passes',0],['passes',4],['parallelism',2],['outputLength',3],['outputLength',1025]]) {
                throws(() => c.argon2id(b(1), new Uint8Array(8), {...opts,[key]:value}));
            }
            throws(() => c.argon2id(new Uint8Array(4097), new Uint8Array(8), opts));
            throws(() => c.argon2id(b(), new Uint8Array(7), opts));
            throws(() => c.argon2id(b(), new Uint8Array(65), opts));
            const pass = b(9,1,2,3,8), salt = new Uint8Array(10).fill(7);
            globalThis.job = c.argon2id(pass.subarray(1,4), salt.subarray(1,9), opts);
            pass.fill(0); salt.fill(0);
            globalThis.result = null;
            job.result.then(value => { globalThis.result = value; });
            throws(() => c.argon2id(b(), new Uint8Array(8), opts));
        "#);
        state
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), state.notified())
        .await
        .unwrap();
    context.with(|ctx| assert!(deliver_crypto(&ctx, &state).unwrap()));
    while rt.execute_pending_job().unwrap() {}
    context.with(|ctx| {
        let result: TypedArray<u8> = ctx.globals().get("result").unwrap();
        let mut expected = [0u8; 32];
        argon2::Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon2::Params::new(9, 1, 1, Some(32)).unwrap(),
        )
        .hash_password_into(&[1, 2, 3], &[7; 8], &mut expected)
        .unwrap();
        assert_eq!(result.as_bytes().unwrap(), expected);
        eval(
            &ctx,
            r#"
            job.cancel();
            globalThis.cancelled = false;
            job = c.argon2id(b(1), new Uint8Array(8), opts);
            job.result.catch(() => {globalThis.cancelled = true;});
            job.cancel(); job.cancel();
        "#,
        );
    });
    while rt.execute_pending_job().unwrap() {}
    context.with(|ctx| eval(&ctx, "assert(cancelled);"));
    state.shutdown();
    super::argon::wait_idle().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn async_context_wakeup_and_promise_pumping() {
    let _serial = super::argon::SERIAL.lock().await;
    let rt = rquickjs::AsyncRuntime::new().unwrap();
    let context = rquickjs::AsyncContext::full(&rt).await.unwrap();
    let state = context.with(|ctx| {
        let state = setup(&ctx);
        eval(&ctx, r#"
            globalThis.settled = false;
            const job = c.argon2id(b(1), new Uint8Array(8),
                {version:19,memoryKiB:32,passes:1,parallelism:1,outputLength:80});
            job.result.then(bytes => { assert(bytes instanceof Uint8Array); assert(bytes.length === 80); settled = true; });
        "#);
        state
    }).await;
    tokio::time::timeout(std::time::Duration::from_secs(10), state.notified())
        .await
        .unwrap();
    context
        .with(|ctx| {
            eval(&ctx, "assert(!settled);");
            assert!(deliver_crypto(&ctx, &state).unwrap());
            assert!(!deliver_crypto(&ctx, &state).unwrap());
        })
        .await;
    while rt.execute_pending_job().await.unwrap() {}
    context.with(|ctx| eval(&ctx, "assert(settled);")).await;
    state.shutdown();
}
