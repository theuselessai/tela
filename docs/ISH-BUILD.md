# Tela iSH development build: i586 / native TLS

This recipe builds **0.1.0-dev-unreleased**, not a published release. It follows
the known-working PLIT v0.1.2 **standalone Rust** iSH route: i586, no SSE, and
vendored native OpenSSL. PLIT is not a QuickJS compatibility proof. Tela's own
QuickJS, crypto, synchronization and PTY paths must pass separately. Docker/QEMU
results are **not iSH/device acceptance**. Main owns packaging and transfer.

## Why this replaces the earlier i686 route

All previous run directories, hashes, containers and caches are preserved:

- `20261004T211916Z-92269`: emulated 32-bit rustc crashed, no binary.
- `20261004T213404Z-97069`: native ARM cross-build worked, but actual CLI PTY
  raw-mode failed `ENOSYS`. Optional `portable-termios` fixed that separately.
- `20261004T220023Z-8872`: i686/SSE2/rustls binary passed Docker and all 50 engine
  tests, but **failed actual iSH**. Help worked; the smoke crashed at `0x591463`,
  `f0 f7 10`, `LOCK NOT [eax]`, in `async_lock::mutex::Mutex::unlock_unchecked`.
  This exact failure was not an SSE opcode.

Main's root `[patch.crates-io]` selects `vendor/event-listener` 5.4.2. Only its
32-bit Linux/musl `full_fence` changes to `LOCK OR [temporary], 0`, with no
`preserves_flags` claim; the full barrier and other targets are retained. See
`vendor/EVENT-LISTENER.md` and the original-source hash manifest. The recipe
snapshots/hashes **vendor as well as crates**, and requires Cargo's selected
normal/build graph to contain that path. This is not a synchronization bypass.

The independent ISA/backend correction uses `i586-unknown-linux-musl`, Pentium
Rust/C flags and `native-tls-vendored`, not i686 with SSE disabled. Production
normal/build graphs must contain **neither ring nor rustls**. Their presence in
the workspace lock or SDK/CLI dev fixtures is not evidence of production use.
`rustls-pki-types` may remain as reqwest's certificate data types; that is not the
rustls backend or ring. Default Mac tests are separate evidence, not 32-bit tests.

## Entry points

From `tela/`:

```sh
ISH_CACHE_RUN=20261004T213404Z-97069 ./scripts/build-ish.sh start
./scripts/build-ish.sh status RUN
# Only once phase=probe-ready-awaiting-approval:
./scripts/build-ish.sh probe RUN
# Once build.exit=0 and phase=ready-for-offline-verification:
./scripts/build-ish.sh verify RUN
./scripts/build-ish.sh ui RUN /absolute/path/to/pairfob-tela/ish-check
```

Omit `ISH_CACHE_RUN` for new caches. Reuse requires matching owner labels and no
running container using either volume. Each run freezes fresh source, records
image digests, toolchain/package versions, flags, dirty source status, per-file
hashes, command snapshots, owned names/CIDs and volume provenance. It never
mounts host home, credentials, `.git`, or the parent workspace. `start` detaches;
inspect `status.txt`, `build.log`, `build.exit` instead of launching duplicates.
Only the recorded owned probe can release the full build gate. No blind retries.

## Toolchain / ABI

- Native ARM64 builder: Rust 1.91.1 Alpine image digest
  `sha256:45c1c35cd364b8055e9e86f8ecd3e8c874b2dcb658d8a4f94b5d111aa0d651a2`.
  `rustup target add i586-unknown-linux-musl` runs inside it only.
- Zig `0.15.2-r0`: `zig cc -target x86-linux-musl -mcpu=pentium`, C flags
  `-march=pentium -mno-sse -mno-sse2 -mno-mmx`. The wrapper filters only cc-rs's
  equivalent `--target=i586-unknown-linux-musl` spelling (Zig expects `x86`).
- Rust flags: `target-cpu=pentium`, static CRT, no SSE/SSE2, self-contained musl,
  and `linker-flavor=ld.lld`. Rust's bundled native `rust-lld` is the final linker;
  there is only one musl/startup-object provider, not Zig plus Rust CRTs.
- `rustc-host.sh` adds dynamic host CRT only to rustc invocations without
  `--target`, so native bindgen can dlopen libclang. `/target/dynamic-host`
  preserves the old static-host cache. A native libclang probe must pass first.
- libclang/headers `21.1.2-r2`, `LIBCLANG_PATH=/usr/lib/llvm21/lib`. Bindgen uses
  the actual Zig x86-musl include search list, explicit i586/no-SSE target flags
  and `-nostdinc`, not ARM headers or GNU pregenerated bindings. Generated musl
  `quickjs-bindings.rs.gz` is retained for inspection.
- Pinned build-base `0.5-r3`, pkgconf `2.5.1-r0`, binutils `2.45.1-r0`, file
  `5.46-r2`. Perl/build tools are required by vendored OpenSSL. Installed Perl,
  QEMU and host OpenSSL versions, plus all APK transitives, are recorded in
  `packages.txt`. APK repository availability is not a bit-reproducibility promise.
- Target `libatomic`: openssl-sys requires `-latomic` for static OpenSSL on
  32-bit Linux; Rust's self-contained musl does not supply it. The first new run
  `20261004T230429Z-22542` reached linking and failed on that missing archive.
  The corrected recipe compiles Zig 0.15.2's **unmodified**
  `/usr/lib/zig/compiler_rt/atomics.zig` into a real Pentium-target static
  `libatomic.a`, adds `-L native=/toolchain`, records source/library hashes and
  symbols, and probes direct 64-bit atomic load/store/fetch-add entry points under
  Pentium QEMU. It does not use an empty archive, host library, source patch or
  weaker synchronization. `build/ish/LICENSE-ZIG` is the upstream 0.15.2 MIT
  license (https://github.com/ziglang/zig/blob/0.15.2/LICENSE); include it in
  the device notices.
- Locked TLS crates: openssl `0.10.81`, openssl-sys `0.9.117`, openssl-src
  `300.6.1+3.6.3` (OpenSSL 3.6.3). This is PLIT's backend strategy, not a claim
  to reproduce its historical versions. `openssl-config.txt` records the actual
  Configure arguments. OpenSSL may contain runtime-dispatched SSE assembly;
  “no-SSE” here means the **required/executed baseline**, not absence of every
  optional SIMD instruction in the ELF. Explicit Pentium execution tests that
  dispatch without masking CPU features or disabling cryptography.

Production invocation:

```sh
cargo build -p tela-cli --target i586-unknown-linux-musl \
  --no-default-features \
  --features native-tls-vendored,tela-engine/generate-bindings,tela-engine/portable-termios \
  --locked --release
```

Both TLS features together intentionally fail compilation. No production
manifest, dependency refresh, crypto parameters or certificate verification
policy is modified by this recipe. Snapshot hashes are checked again after
compilation. `production-tree.txt`, `patch-selection.txt`, `tls-selection.txt`
and `smoke-tree.txt` record the actual selected packages.

## Isolated i586 verification, not SSE-backed SDK tests

`build/ish/smoke-Cargo.toml` is a separate workspace depending on the public
Engine with defaults disabled and the same three optional features, plus pinned
Tokio, async-lock and native-tls. It applies the same root vendor patch. It never
builds engine/CLI dev dependencies (those TLS fixtures enable ring). A copy of
the production lock is pruned **offline** for this graph; the production lock is
not edited. `smoke-Cargo.lock` records that isolated resolution.

`ish-smoke` verifies:

- Public Engine / QuickJS Unicode/JSON, UTF-8, secure randomness, known SHA-256,
  X25519 agreement, ChaCha20-Poly1305 roundtrip and tamper rejection.
- Public async Argon2id: **64 MiB, 3 passes, parallelism 1, 80 bytes**, without
  weakening application/PAKE parameters. Timing is emulator-only.
- Four contending async-lock workers, 1,000 protected-data handoffs each, an
  initially held lock, paired-data invariant, final count and bounded completion
  channels. This exercises the notification slow path and detects lost wakes;
  it is not a formal concurrency proof.
- Public Engine `fetch` over native TLS: a matching localhost certificate chains
  to a test CA via `SSL_CERT_FILE`; hostname mismatch and untrusted certificate
  fail. No `danger_accept_invalid_*` option or certificate-validation bypass.
  Test-only keys/certificates live under `tls-fixtures/`, expire after two days,
  and **must not be packaged for the device**. The verifier needs those fixtures
  and environment variables; the bare diagnostic binary alone is not the complete
  TLS harness.

The build explicitly runs `qemu-i386 -cpu pentium` for minimal probe, CLI help,
run help and the entire isolated smoke. `pentium-verification.log/.exit` is the
explicit no-SSE CPU execution evidence, not a claim based on ELF machine number
or raw byte searching. Static checks require ELF32/Intel 80386 with no INTERP or
NEEDED for Tela and the smoke. `verify` uses `disassemble.py` with LLVM objdump (on this Mac, `xcrun
llvm-objdump`) to decode both artifacts, retain compressed full disassembly and
require zero decoded `LOCK NOT` plus notification-symbol `LOCK OR [memory], 0`
paths. `disassembly-review.json` records symbol/address/context and artifact
hashes. This does not infer whole-CPU compatibility from absence of a byte pattern.

`verify` repeats hashes/help/smoke in a clean linux/386 Alpine container with
network disabled, read-only root/artifacts, `/tmp` tmpfs, and
`QEMU_CPU=pentium`. The pinned verifier image is
`sha256:02112cb9761532f596291bee3c8a30f5b383e3020bc50a143e0f1bffdd40069f`.
Loopback TLS fixtures need no external network or system trust modification.
There is deliberately no `full-tests.exit` pretending the 50 SDK tests ran on
i586. Reuse the already recorded Mac tests for defaults, keeping claims separate.

`ui` runs the intended entry point **`/artifact/tela run /check`**, under a real
100x30 PTY in a network-disabled/read-only linux/386 container with
`QEMU_CPU=pentium`. Only artifacts and main's unchanged `ish-check` directory
are mounted. It requires the rendered `PASS: QuickJS/UI`, sends `q`, and requires
both client/container exit 0. It records transcript, input hashes, command and
container state. Previous UI logs cannot be silently overwritten. The optional
interactive Argon2 timing is not triggered here (the isolated smoke covers it).

After core plus PTY success the small first-party archive is
`tela-0.1.0-dev-unreleased-i586-musl.tar.gz`. Main supplies application packaging,
third-party notices and final transfer checksums. Do not treat this internal
archive as a published or iSH-accepted release.

## Native TLS certificate roots and device handoff

Vendored OpenSSL provides the library, **not a bundled public trust store**.
iSH needs a current OS CA bundle (on Alpine normally
`/etc/ssl/certs/ca-certificates.crt`, from `ca-certificates`). Before network use,
check that the bundle exists and is nonempty. If OpenSSL's compiled default path
does not locate the Alpine bundle, explicitly select the existing trusted bundle:

```sh
export SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
test -s "$SSL_CERT_FILE"
```

Main/user owns any package installation. Do not copy the test CA into OS trust,
ship fixture keys, use insecure TLS flags, or claim live service interoperability
from localhost tests. Hostname verification remains enabled. Offline UI/crypto
checks do not require CA roots or credentials.

Transfer main's final archive into a fresh directory, verify its checksum, run
`./tela --help`, then `./tela run ./check`. Keep the known-working installation
untouched. Report actual iSH version, success/errors/dmesg, terminal behavior and
optional Argon2 device timing. **Stop for user device retest**: no new pairing,
credential copies, live cloud, VM restart/prune, global installs, publication or
LAN-server changes are owned by this worker.

At a deadline retain a run-local `CHECKPOINT.md` with exact phase, resource IDs,
verification limits and next command. Never delete old evidence or stop unrelated
containers. Cleanup requires separate authority.
