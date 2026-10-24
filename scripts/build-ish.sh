#!/bin/bash
# Detached native ARM64 cross-build; preserved checkpoints survive agent deadlines.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
IMAGE='rust@sha256:45c1c35cd364b8055e9e86f8ecd3e8c874b2dcb658d8a4f94b5d111aa0d651a2'
VERIFY_IMAGE='i386/alpine@sha256:02112cb9761532f596291bee3c8a30f5b383e3020bc50a143e0f1bffdd40069f'
ACTION=${1:-start}
case "$ACTION" in
  start)
    RUN="$(date -u +%Y%m%dT%H%M%SZ)-$$"
    OUT="$ROOT/dist/ish/$RUN"
    NAME="tela-ish-build-$RUN"
    mkdir -p "$OUT"
    printf '%s\n' "$RUN" > "$ROOT/dist/ish/latest-run.txt"
    printf '%s\n' "$NAME" > "$OUT/container-name.txt"
    printf '%s\n' "$IMAGE" > "$OUT/image.txt"
    printf '%s\n' "$VERIFY_IMAGE" > "$OUT/verify-image.txt"
    date -u > "$OUT/started-at.txt"
    cp "$ROOT/build/ish/build.sh" "$OUT/build-recipe.sh"
    cp -R "$ROOT/build/ish/compiler-rt-atomic" "$OUT/compiler-rt-atomic"
    cp "$ROOT/build/ish/"{zig-cc.sh,zig-ar.sh,rustc-host.sh,host-probe.rs,probe.rs,smoke.rs,smoke-Cargo.toml,tls-fixtures.sh,LICENSE-ZIG} "$OUT/"
    cp "$0" "$OUT/launcher.sh"
    (cd "$OUT" && shasum -a 256 build-recipe.sh launcher.sh zig-cc.sh zig-ar.sh rustc-host.sh host-probe.rs probe.rs smoke.rs smoke-Cargo.toml tls-fixtures.sh LICENSE-ZIG > recipe-sha256.txt)
    (cd "$OUT" && find compiler-rt-atomic -type f -exec shasum -a 256 {} \; >> recipe-sha256.txt)
    git -C "$ROOT" rev-parse HEAD > "$OUT/source-head.txt"
    git -C "$ROOT" status --short -- Cargo.toml Cargo.lock crates vendor > "$OUT/source-status.txt"
    docker image inspect "$IMAGE" --format '{{.Id}} {{json .RepoDigests}}' > "$OUT/image-inspect.txt"
    docker image inspect "$VERIFY_IMAGE" --format '{{.Id}} {{json .RepoDigests}}' > "$OUT/verify-image-inspect.txt"
    # Freeze fresh compilation inputs even when reusing an older dependency cache.
    mkdir "$OUT/source"
    cp "$ROOT/"{Cargo.toml,Cargo.lock,LICENSE-MIT,LICENSE-APACHE} "$OUT/source/"
    cp -R "$ROOT/crates" "$ROOT/vendor" "$OUT/source/"
    CACHE_NAME="$NAME"
    if [ -n "${ISH_CACHE_RUN:-}" ]; then
      [[ "$ISH_CACHE_RUN" =~ ^[0-9]{8}T[0-9]{6}Z-[0-9]+$ ]] || exit 2
      CACHE_NAME="tela-ish-build-$ISH_CACHE_RUN"
      for KIND in cargo target; do
        VOL="$CACHE_NAME-$KIND"
        [ "$(docker volume inspect --format '{{index .Labels "tela.ish.owner"}}' "$VOL")" = "$CACHE_NAME" ]
        [ -z "$(docker ps -q --filter "volume=$VOL")" ]
        docker volume inspect "$VOL" > "$OUT/cache-$KIND-inspect.json"
        printf '%s\n' "$VOL" >> "$OUT/volumes.txt"
      done
      printf 'Reused inactive owner-checked cache from %s; source is fresh, not reused.\n' "$ISH_CACHE_RUN" > "$OUT/cache-provenance.txt"
    else
      for KIND in cargo target; do
        docker volume create --label tela.ish.owner="$NAME" "$NAME-$KIND" >> "$OUT/volumes.txt"
      done
    fi
    CID=$(docker create --platform linux/arm64 --name "$NAME" \
      --label tela.ish.owner="$NAME" \
      --mount "type=bind,src=$OUT/source,dst=/src,readonly" \
      --mount "type=bind,src=$OUT,dst=/out" \
      --mount "type=volume,src=$CACHE_NAME-cargo,dst=/cargo" \
      --mount "type=volume,src=$CACHE_NAME-target,dst=/target" \
      "$IMAGE" sh /out/build-recipe.sh)
    printf '%s\n' "$CID" > "$OUT/container-id.txt"
    # Only clean up this container on an interrupted launch. A successful launch
    # intentionally leaves it inspectable for status, logs, and reconciliation.
    trap 'docker stop "$CID" >/dev/null 2>&1 || true; exit 130' INT TERM
    docker start "$CID" >/dev/null
    trap - INT TERM
    printf 'Started %s\nOutput: %s\nStatus: %s status %s\n' "$NAME" "$OUT" "$0" "$RUN"
    ;;
  status|probe|verify|ui)
    RUN=${2:-$(< "$ROOT/dist/ish/latest-run.txt")}
    [[ "$RUN" =~ ^[0-9]{8}T[0-9]{6}Z-[0-9]+$ ]] || { echo 'Invalid run ID' >&2; exit 2; }
    OUT="$ROOT/dist/ish/$RUN"
    CID=$(< "$OUT/container-id.txt")
    NAME=$(< "$OUT/container-name.txt")
    [ "$(docker inspect --format '{{index .Config.Labels "tela.ish.owner"}}' "$CID")" = "$NAME" ]
    docker inspect --format '{{.Name}} {{.State.Status}} exit={{.State.ExitCode}} started={{.State.StartedAt}} finished={{.State.FinishedAt}}' "$CID"
    if [ "$ACTION" = status ]; then
      [ ! -f "$OUT/status.txt" ] || head -n 1 "$OUT/status.txt"
      [ ! -f "$OUT/build.log" ] || tail -n 25 "$OUT/build.log"
      exit 0
    fi
    if [ "$ACTION" = probe ]; then
      [ "$(< "$OUT/status.txt")" = phase=probe-ready-awaiting-approval ]
      PNAME="$NAME-probe-$(date -u +%H%M%S)"
      printf '%s\n' "$PNAME" > "$OUT/probe-container-name.txt"
      PCID=$(docker create --platform linux/386 --network none --read-only \
        --name "$PNAME" --label tela.ish.owner="$PNAME" --env QEMU_CPU=pentium \
        --mount "type=bind,src=$OUT,dst=/artifact,readonly" \
        "$VERIFY_IMAGE" timeout 30 /artifact/cross-probe)
      printf '%s\n' "$PCID" > "$OUT/probe-container-id.txt"
      docker start -a "$PCID" > "$OUT/probe-verification.log" 2>&1 || true
      RC=$(docker inspect --format '{{.State.ExitCode}}' "$PCID")
      printf '%s\n' "$RC" > "$OUT/probe-verification.exit"
      [ "$RC" = 0 ]
      touch "$OUT/probe-approved"
      echo 'Minimal ELF executed successfully in linux/386; full build released.'
      exit 0
    fi
    [ "$(docker inspect --format '{{.State.Status}}' "$CID")" = exited ]
    [ "$(< "$OUT/build.exit")" = 0 ]
    if [ "$ACTION" = ui ]; then
      [ "$(< "$OUT/verification.exit")" = 0 ]
      [ "$(< "$OUT/pentium-verification.exit")" = 0 ]
      [ "$(< "$OUT/disassembly-verification.exit")" = 0 ]
      [ -n "${3:-}" ] || { echo 'ui requires the ish-check folder path' >&2; exit 2; }
      [ ! -e "$OUT/ui-verification.log" ] || { echo 'Preserve previous UI attempt before retrying' >&2; exit 2; }
      cp "$ROOT/build/ish/verify-pty.py" "$OUT/verify-pty.py"
      (cd "$OUT" && shasum -a 256 verify-pty.py > ui-recipe-sha256.txt)
      python3 "$OUT/verify-pty.py" "$OUT" "$3"
      printf 'phase=verified-in-docker-pty-not-on-ish\n' > "$OUT/status.txt"
      COPYFILE_DISABLE=1 tar -czf "$OUT/tela-0.1.0-dev-unreleased-i586-musl.tar.gz" -C "$OUT" tela LICENSE-MIT LICENSE-APACHE LICENSE-ZIG LICENSE-LLVM-ATOMIC
      (cd "$OUT" && shasum -a 256 tela-0.1.0-dev-unreleased-i586-musl.tar.gz > archive-sha256.txt)
      exit 0
    fi
    cp "$0" "$OUT/verification-launcher.sh"
    cp "$ROOT/build/ish/disassemble.py" "$OUT/disassemble.py"
    (cd "$OUT" && shasum -a 256 verification-launcher.sh disassemble.py > verification-recipe-sha256.txt)
    python3 "$OUT/disassemble.py" "$OUT"
    VERIFY_NAME="$NAME-verify-$(date -u +%H%M%S)"
    printf '%s\n' "$VERIFY_NAME" > "$OUT/verify-container-name.txt"
    VCID=$(docker create --platform linux/386 --network none --read-only \
      --name "$VERIFY_NAME" --label tela.ish.owner="$VERIFY_NAME" --env QEMU_CPU=pentium \
      --tmpfs /tmp:rw,nosuid,nodev --mount "type=bind,src=$OUT,dst=/artifact,readonly" \
      "$VERIFY_IMAGE" sh -ec 'cd /artifact; export SSL_CERT_FILE=/artifact/tls-fixtures/ca.pem SSL_CERT_DIR=/artifact/tls-fixtures/empty-roots ISH_TLS_FIXTURES=/artifact/tls-fixtures; sha256sum -c SHA256SUMS; timeout 30 ./tela --help; timeout 30 ./tela run --help; timeout 300 ./ish-smoke')
    printf '%s\n' "$VCID" > "$OUT/verify-container-id.txt"
    trap 'docker stop "$VCID" >/dev/null 2>&1 || true; exit 130' INT TERM
    docker start -a "$VCID" > "$OUT/verification.log" 2>&1 || true
    RC=$(docker inspect --format '{{.State.ExitCode}}' "$VCID")
    printf '%s\n' "$RC" > "$OUT/verification.exit"
    # Isolated i586 proof only: SDK dev tests enable rustls/ring and are not built.
    trap - INT TERM
    if [ "$RC" = 0 ]; then
      printf 'phase=core-verified-awaiting-real-pty\n' > "$OUT/status.txt"
    fi
    printf 'Verification exit=%s; log: %s/verification.log\n' "$RC" "$OUT"
    exit "$RC"
    ;;
  *) echo "Usage: $0 [start|status [RUN]|probe [RUN]|verify [RUN]|ui RUN ISH_CHECK_PATH]" >&2; exit 2 ;;
esac
