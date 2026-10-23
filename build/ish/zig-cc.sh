#!/bin/sh
# True i586/Pentium baseline: no SSE, SSE2 or MMX; native TLS replaces ring.
# C compilation only. Final linking uses Rust's bundled rust-lld so Rust's
# self-contained musl CRT/libc remains the sole runtime provider.
# cc-rs detects clang and appends Rust's triple, which overrides Zig's target
# but uses an architecture spelling Zig does not accept. Keep our x86-musl
# spelling of precisely the same target (do not filter arbitrary other targets).
for arg do
  shift
  case "$arg" in
    --target=i586-unknown-linux-musl) ;;
    *) set -- "$@" "$arg" ;;
  esac
done
exec zig cc -target x86-linux-musl -mcpu=pentium -mno-sse -mno-sse2 -mno-mmx -fno-sanitize=all "$@"
