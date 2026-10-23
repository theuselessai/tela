# Tela.crypto

A small RustCrypto-backed native API, **not WebCrypto**. Available only with the
`crypto` manifest permission once registered by the host. No protocol constants,
transcripts, domain separation, service addresses, or pairing orchestration are
provided by this SDK.

## Bytes and errors

All byte inputs are actual `Uint8Array` objects. Arrays, strings, `ArrayBuffer`,
`DataView`, and other typed arrays are rejected, without coercion. Subarrays use
exactly their byte offset and length, even if JS shadows their `length` property.
Detached views are rejected. Outputs are fresh owned `Uint8Array` objects.
Ordinary byte inputs and outputs are each limited to 1,048,576 bytes. Integer
arguments must be finite integral JS numbers, not strings or coerced values.

Validation failures throw synchronously. Native error messages are fixed and do
not include input bytes. Cryptographic verification failures return `false` for
Ed25519, while AEAD authentication and noncontributory X25519 failures throw.
Exceptions thrown by application-defined option getters remain application errors.

## Synchronous operations

```ts
Tela.crypto.randomBytes(length: number): Uint8Array
Tela.crypto.sha256(data: Uint8Array): Uint8Array
Tela.crypto.hmacSha256(key: Uint8Array, data: Uint8Array): Uint8Array
Tela.crypto.hkdfSha256(
  ikm: Uint8Array, salt: Uint8Array | null, info: Uint8Array, length: number
): Uint8Array
Tela.crypto.timingSafeEqual(a: Uint8Array, b: Uint8Array): boolean
Tela.crypto.ed25519Verify(
  publicKey32: Uint8Array, message: Uint8Array, signature64: Uint8Array
): boolean
```

- Random bytes use OS entropy; length is 0 through 1,048,576.
- SHA-256 and HMAC-SHA-256 return 32 bytes.
- HKDF is RFC 5869 HKDF-SHA-256, including extract and expand. `null` salt means
  absent salt. Output length is 0 through 8160 (255 × 32); never truncated/clamped.
- Equality uses `subtle` byte comparison; unequal lengths return `false`. Lengths
  are not secret and this is not a whole-runtime constant-time guarantee.
- Ed25519 uses `ed25519-dalek`'s `Verifier::verify`, **not** `verify_strict`.
  This includes that provider's non-strict small-order policy. Incorrect key or
  signature lengths throw; invalid encodings/equations return `false`.

## Opaque native objects

`P256Scalar`, `P256Point`, `X25519Private`, and `ChaChaKey` are distinct native
object types, not numeric IDs, JSON records, or publicly constructible classes.
Each exposes `dispose(): void`, which is idempotent. All other uses after disposal
throw. Wrong-kind, forged, and cross-context handles are rejected.

At most 256 live, undisposed native objects may exist per context, across all four
types. Dispose promptly rather than relying on GC timing. GC is a fallback that
drops the native allocation; shutdown disposes all remaining native objects.
Disposed JS shells do not consume native live-object slots.

### P-256

```ts
const p = Tela.crypto.p256;
p.scalarRandomNonzero(): P256Scalar
p.scalarImport32(canonicalBigEndian32: Uint8Array): P256Scalar
p.scalarReduceBE(bigEndian: Uint8Array): P256Scalar
p.scalarExport32(scalar: P256Scalar): Uint8Array
p.pointImport65(uncompressed: Uint8Array): P256Point
p.pointExport65(point: P256Point): Uint8Array
p.baseMul(scalar: P256Scalar): P256Point
p.mul(point: P256Point, scalar: P256Scalar): P256Point
p.add(left: P256Point, right: P256Point): P256Point
p.sub(left: P256Point, right: P256Point): P256Point
p.isIdentity(point: P256Point): boolean
```

Random scalars use canonical rejection sampling in `1..n-1`. Import accepts
exactly 32 big-endian bytes strictly less than the group order, including zero.
Reduction accepts 0 through 64 big-endian bytes modulo the **group order**, not
the coordinate-field modulus. Empty input reduces to zero. Export is 32 bytes.

Point import accepts only canonical on-curve `04 || X32 || Y32` encodings.
Compressed, hybrid, off-curve, noncanonical-coordinate, and identity encodings
are rejected. Point export rejects identity. Arithmetic may produce and retain
identity internally, including multiplication by zero and subtraction of equal
points. Callers must apply their own protocol rules (for example, rejecting an
identity after unmasking); the generic SDK must not silently alter these rules.

### X25519

```ts
const x = Tela.crypto.x25519;
x.generate(): X25519Private
x.importPrivate32(bytes: Uint8Array): X25519Private
x.publicKey(privateKey: X25519Private): Uint8Array
x.derive(privateKey: X25519Private, peerPublicKey32: Uint8Array): Uint8Array
```

Private inputs are exactly 32 bytes; clamping and peer decoding follow
`x25519-dalek`. Public keys and derived secrets are 32 bytes. Derivation rejects
an all-zero shared secret, including low-order peer inputs. There is no private
key export operation.

### ChaCha20-Poly1305

```ts
const a = Tela.crypto.chacha20poly1305;
a.importKey32(bytes: Uint8Array): ChaChaKey
a.seal(key: ChaChaKey, nonce12: Uint8Array, aad: Uint8Array,
       plaintext: Uint8Array): Uint8Array
a.open(key: ChaChaKey, nonce12: Uint8Array, aad: Uint8Array,
       ciphertextAndTag: Uint8Array): Uint8Array
```

This is IETF ChaCha20-Poly1305 with a 32-byte key and a 12-byte nonce. `seal`
returns ciphertext followed by its 16-byte tag, **without a nonce prefix**.
`open` requires at least 16 bytes and returns no plaintext on authentication
failure. To keep outputs within the ordinary byte bound, plaintext passed to
`seal` is limited to 1,048,560 bytes. AAD is independently bounded to 1 MiB.
Nonce uniqueness is the caller's responsibility. These operations deliberately
remain synchronous so the SDK does not introduce asynchronous nonce races.

## Argon2id

```ts
const job = Tela.crypto.argon2id(passwordBytes, saltBytes, {
  version: 19,
  memoryKiB: 65536,
  passes: 3,
  parallelism: 1,
  outputLength: 80,
});
const derived: Uint8Array = await job.result;
// Alternatively: job.cancel();
```

Supported options, all required:

| Option/input | Accepted values |
| --- | --- |
| version | 19 (Argon2 v1.3) |
| memoryKiB | 8 through 65536 |
| passes | 1 through 3 |
| parallelism | exactly 1 |
| outputLength | 4 through 1024 bytes |
| password | 0 through 4096 bytes |
| salt | 8 through 64 bytes |

The exact requested memory cost is passed to Argon2, including nonmultiples of
four; its standard block-layout rounding does not change the cost committed to
by the hash. Unsupported parameters are never clamped. The exact
65536-KiB / 3-pass / 1-lane / 80-byte configuration is supported and tested.

Inputs are copied before return. The return value is
`{ result: Promise<Uint8Array>, cancel(): void }`. Invalid arguments, missing
worker runtime, or busy admission throw synchronously; computational failures
reject `result`. Attach a rejection handler when cancellation is possible.

Only **one admitted computation per process** runs at a time; there is no queue.
Each context additionally has only one pending delivery slot. A completed but
undelivered result prevents another admission in that context until it is
cancelled or delivered. Other contexts may admit after blocking work finishes.

`cancel()` is idempotent and immediately rejects an unsettled promise. It does
not promise interruption of Argon2. The blocking closure retains the global
permit through computation and cleanup even after cancellation or host teardown.
A cancelled result is wiped rather than delivered. No JS values enter workers.

## Host integration (Engine-owned)

The stock Engine now registers this module when the manifest grants `crypto`.
Its lifecycle integration uses these crate-local APIs in `native::crypto`:

```rust
// Inside ctx.with, after Tela exists, and only with permission "crypto":
let crypto: Arc<CryptoRuntime> = register_crypto(&ctx)?;

// In the main event select (and its nonblocking drain loop):
crypto.notified().await;

// Inside the owning context, after a notification or during manual pumping:
let delivered: bool = deliver_crypto(&ctx, &crypto)?;

// Before dropping the context, on shutdown and from Engine::drop:
crypto.shutdown();
```

Implemented wiring (retain these invariants when changing the event loop):

1. Store `Option<Arc<CryptoRuntime>>` in Engine. Do not expose `Tela.crypto` when
   permission is absent. Registration requires `Tela` to exist and rejects
   duplicate registration when its `crypto` property is already present.
2. Add a `crypto.notified()` branch alongside native completions in **both**
   event-loop selects. When disabled, use a permanently pending future. `Notify`
   stores a wakeup if a worker finishes before selection begins.
3. After a wakeup, call `deliver_crypto` in `ctx.with`, then execute pending JS
   jobs, drain `Tela.dispatch` actions, run reducers, and check quit, following
   the existing native completion handler. A spurious/stale notification is
   harmless: delivery returns `false` when no result is ready.
4. Also drain via `deliver_crypto` from `Engine::execute_pending_jobs`, before
   and after JS pumping, so headless/manual pumping works without `run()`.
5. Invoke `shutdown()` from both explicit Engine shutdown and Drop, before the
   context is released. Serialize delivery with context teardown as for existing
   native completion delivery. Shutdown itself does not access JS, settle
   promises, or interrupt workers. It clears pending output and native secrets.

`deliver_crypto` rejects the wrong context and is a no-op after shutdown. The
internal `__tela_crypto_complete__` JS resolver follows the existing Engine
completion convention; it is not public SDK. Argon2 uses a bounded Rust result
slot and `Notify`, **not** the NativeModule 64-KiB JSON parameter bridge. All
public crypto inputs and results remain typed binary values.

## Security limits and verification

Owned secret byte buffers, scalar and derived-point handles, Argon2 scratch memory, and cancelled
outputs are zeroized where supported. Provider-internal temporaries and native
stack/register copies are not comprehensively erased. JS can access input,
derived, and exported scalar bytes; JS GC/string copies cannot reliably be
wiped. Scalar export is intentionally available and is not a nonextractability
guarantee. This API does not claim full protocol/runtime constant-time behavior,
hostile-app isolation, nonce management, or protocol correctness.

Synthetic tests exercise actual QuickJS bindings, published primitive vectors,
byte-view offsets, canonical/curve/identity rules, noncontributory X25519, invalid
signatures/tags/AAD, native type/disposal/GC bounds, wrong contexts, and Argon2
admission/cancellation/teardown. Run from `tela/`:

```sh
SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk \
  cargo test --offline --locked -p tela-engine native::crypto
```

Engine permission/event-loop wiring and application-level frozen protocol
vectors are a separate integration responsibility; these module tests do not
claim to verify those paths.
