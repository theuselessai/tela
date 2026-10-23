#!/bin/sh
# Cargo's --target build separates target RUSTFLAGS from host build scripts.
# Native musl build scripts must be dynamic for bindgen's libclang dlopen.
RUSTC=$1
shift
for arg do
  case "$arg" in
    --target|--target=*) exec "$RUSTC" "$@" ;;
  esac
done
exec "$RUSTC" "$@" -C target-feature=-crt-static
