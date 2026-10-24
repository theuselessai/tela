#!/bin/sh
# Native ARM64 compiler; only output executables are i586, without SSE.
set -eu
exec > /out/build.log 2>&1
trap 'rc=$?; printf "%s\n" "$rc" > /out/build.exit; date -u > /out/finished-at.txt' EXIT
printf 'phase=packages\n' > /out/status.txt
apk add --no-cache zig=0.15.2-r0 clang21-libclang=21.1.2-r2 clang21-headers=21.1.2-r2 \
  build-base=0.5-r3 pkgconf=2.5.1-r0 binutils=2.45.1-r0 file=5.46-r2 perl qemu-i386 openssl
apk info -v | sort > /out/packages.txt
rustc -vV > /out/rustc.txt
cargo -vV > /out/cargo.txt
zig version > /out/zig-version.txt
zig env > /out/zig-env.txt
qemu-i386 --version > /out/qemu-version.txt
[ "$(rustc -vV | awk '/^host:/ {print $2}')" = aarch64-unknown-linux-musl ]
TARGET=i586-unknown-linux-musl
rustup target add "$TARGET"
rustup show > /out/rustup.txt
export CARGO_HOME=/cargo CARGO_TARGET_DIR=/target/dynamic-host CARGO_BUILD_JOBS=2
export RUSTFLAGS='-C target-cpu=pentium -C target-feature=+crt-static,-sse,-sse2 -C link-self-contained=yes -C linker-flavor=ld.lld -L native=/toolchain'
mkdir -p /work /toolchain
# openssl-sys explicitly links libatomic for static OpenSSL on 32-bit Linux.
# LLVM's complete unmodified compiler-rt atomic runtime includes the generic
# __atomic_is_lock_free query required by OpenSSL (Zig's atomics.zig omits it).
# Never substitute host code, an empty archive, or a hand-written atomic shim.
zig cc -target x86-linux-musl -mcpu=pentium -O2 -fPIC -fno-sanitize=all \
  -I/out/compiler-rt-atomic -c /out/compiler-rt-atomic/atomic.c -o /toolchain/atomic.o
zig ar rcs /toolchain/libatomic.a /toolchain/atomic.o
sha256sum /out/compiler-rt-atomic/* /toolchain/libatomic.a > /out/atomic-runtime-sha256.txt
cp /out/compiler-rt-atomic/LICENSE.TXT /out/LICENSE-LLVM-ATOMIC
nm -g /toolchain/libatomic.a > /out/atomic-runtime-symbols.txt
cp /out/zig-cc.sh /toolchain/zig-cc
cp /out/zig-ar.sh /toolchain/zig-ar
cp /out/rustc-host.sh /toolchain/rustc-host
chmod +x /toolchain/*
export RUSTC_WRAPPER=/toolchain/rustc-host
printf 'phase=native-libclang-probe\n' > /out/status.txt
/toolchain/rustc-host "$(command -v rustc)" --edition 2021 /out/host-probe.rs -o /out/host-probe
/out/host-probe > /out/host-probe.log
readelf -h -l /out/host-probe > /out/host-probe-readelf.txt
export CC_i586_unknown_linux_musl=/toolchain/zig-cc
export AR_i586_unknown_linux_musl=/toolchain/zig-ar
export CARGO_TARGET_I586_UNKNOWN_LINUX_MUSL_LINKER="$(rustc --print sysroot)/lib/rustlib/aarch64-unknown-linux-musl/bin/rust-lld"
export CFLAGS_i586_unknown_linux_musl='-march=pentium -mno-sse -mno-sse2 -mno-mmx'
export LIBCLANG_PATH=/usr/lib/llvm21/lib
printf '' | /toolchain/zig-cc -E -v -xc - > /out/zig-preprocessor.txt 2>&1
awk '/#include <...> search starts here:/{p=1;next} /End of search list/{p=0} p && /^ /{print $1}' \
  /out/zig-preprocessor.txt > /out/target-includes.txt
[ -s /out/target-includes.txt ]
grep -q '/generic-musl' /out/target-includes.txt
grep -q '/x86-linux-musl' /out/target-includes.txt
BINDGEN_EXTRA_CLANG_ARGS_i586_unknown_linux_musl='--target=i586-unknown-linux-musl -march=pentium -mno-sse -mno-sse2 -mno-mmx -nostdinc'
while IFS= read -r include; do
  BINDGEN_EXTRA_CLANG_ARGS_i586_unknown_linux_musl="$BINDGEN_EXTRA_CLANG_ARGS_i586_unknown_linux_musl -isystem $include"
done < /out/target-includes.txt
export BINDGEN_EXTRA_CLANG_ARGS_i586_unknown_linux_musl
rustc --print cfg --target "$TARGET" $RUSTFLAGS > /out/target-cfg.txt
FEATURES=native-tls-vendored,tela-engine/generate-bindings,tela-engine/portable-termios
{
  printf 'version=0.1.0-dev-unreleased\ntarget=%s\n' "$TARGET"
  env | sort | grep -E '^(RUSTFLAGS|RUSTC_WRAPPER|CARGO_|CC_|AR_|CFLAGS_|LIBCLANG|BINDGEN_)'
  printf 'build=cargo build -p tela-cli --target %s --no-default-features --features %s --locked --release\n' "$TARGET" "$FEATURES"
} > /out/build-metadata.txt
check_elf() {
  file "$1" >> /out/file.txt
  readelf -h -l -d -A "$1" > "/out/$2-readelf.txt"
  readelf -h "$1" | grep -q 'Class:.*ELF32'
  readelf -h "$1" | grep -q 'Machine:.*Intel 80386'
  if readelf -l "$1" | grep -q INTERP || readelf -d "$1" | grep -q NEEDED; then
    echo "Not static: $1"; exit 1
  fi
}
printf 'phase=minimal-cross-probe\n' > /out/status.txt
rustc --edition 2021 --target "$TARGET" $RUSTFLAGS -C linker="$CARGO_TARGET_I586_UNKNOWN_LINUX_MUSL_LINKER" \
  /out/probe.rs -o /out/cross-probe
check_elf /out/cross-probe cross-probe
timeout 30 qemu-i386 -cpu pentium /out/cross-probe > /out/pentium-probe.log 2>&1
printf 'phase=probe-ready-awaiting-approval\n' > /out/status.txt
while [ ! -f /out/probe-approved ]; do sleep 2; done
[ "$(head -n 1 /out/probe-verification.exit)" = 0 ]
cp /src/Cargo.toml /src/Cargo.lock /work/
cp -R /src/crates /src/vendor /work/
cd /work
find Cargo.toml Cargo.lock crates vendor -type f -exec sha256sum {} \; | sort -k2 > /out/source-sha256.txt
sha256sum -c /out/source-sha256.txt > /out/source-recheck.txt
printf 'phase=build\n' > /out/status.txt
cargo build -p tela-cli --target "$TARGET" --no-default-features --features "$FEATURES" --locked --release
cargo tree --locked -p tela-cli --target "$TARGET" --no-default-features --features "$FEATURES" \
  --edges normal,build > /out/production-tree.txt
# Check only the selected production graph, not dev fixtures or lockfile presence.
! grep -E '(^|[[:space:]])(ring|rustls) v' /out/production-tree.txt
grep -F 'event-listener v5.4.2 (/work/vendor/event-listener)' /out/production-tree.txt > /out/patch-selection.txt
grep -E 'openssl(-sys|-src)? v|native-tls v' /out/production-tree.txt > /out/tls-selection.txt
find "$CARGO_TARGET_DIR/$TARGET/release/build" -path '*/openssl-sys-*/out/openssl-build/build/src/configdata.pm' \
  -exec perl {} --dump \; > /out/openssl-config.txt
cp "$CARGO_TARGET_DIR/$TARGET/release/tela" /out/tela
cp /src/LICENSE-MIT /src/LICENSE-APACHE /out/
check_elf /out/tela tela
printf 'phase=compile-isolated-smoke\n' > /out/status.txt
mkdir -p /smoke/src
cp /out/smoke-Cargo.toml /smoke/Cargo.toml
cp /out/smoke.rs /smoke/src/main.rs
cp /work/Cargo.lock /smoke/Cargo.lock
cd /smoke
# Prune the copied lock to this isolated graph OFFLINE, with exact direct versions;
# never resolve/update the production workspace or pull its dev TLS fixtures.
cargo build --offline --release --target "$TARGET"
cp Cargo.lock /out/smoke-Cargo.lock
cargo tree --offline --locked --target "$TARGET" --edges normal,build > /out/smoke-tree.txt
! grep -E '(^|[[:space:]])(ring|rustls) v' /out/smoke-tree.txt
grep -F 'event-listener v5.4.2 (/work/vendor/event-listener)' /out/smoke-tree.txt >> /out/patch-selection.txt
cp "$CARGO_TARGET_DIR/$TARGET/release/ish-smoke" /out/ish-smoke
check_elf /out/ish-smoke ish-smoke
find "$CARGO_TARGET_DIR/$TARGET/release/build" -path '*/rquickjs-sys-*/out/bindings.rs' \
  -exec sh -c 'gzip -c "$1" > /out/quickjs-bindings.rs.gz' sh {} \;
sh /out/tls-fixtures.sh > /out/tls-fixtures-generation.log 2>&1
cd /out
sha256sum tela ish-smoke LICENSE-MIT LICENSE-APACHE LICENSE-ZIG LICENSE-LLVM-ATOMIC > SHA256SUMS
printf 'phase=pentium-verification\n' > /out/status.txt
export SSL_CERT_FILE=/out/tls-fixtures/ca.pem SSL_CERT_DIR=/out/tls-fixtures/empty-roots ISH_TLS_FIXTURES=/out/tls-fixtures
# Explicit emulator CPU, not an assumption about Docker's binfmt CPU.
( timeout 30 qemu-i386 -cpu pentium ./tela --help && \
  timeout 30 qemu-i386 -cpu pentium ./tela run --help && \
  timeout 300 qemu-i386 -cpu pentium ./ish-smoke ) > /out/pentium-verification.log 2>&1
printf '0\n' > /out/pentium-verification.exit
cd /work
sha256sum -c /out/source-sha256.txt > /out/source-postbuild-recheck.txt
printf 'phase=ready-for-offline-verification\n' > /out/status.txt
