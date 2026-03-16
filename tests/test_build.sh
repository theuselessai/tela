#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "$0")/.." && pwd)"
source "$DIR/tests/helpers.sh"

echo "=== test_build ==="

if [ -f "$TELA_BIN" ]; then
  echo "  PASS: tela binary exists at $TELA_BIN"
  ((PASSED++))
else
  echo "  FAIL: tela binary not found at $TELA_BIN"
  ((FAILED++))
fi

"$TELA_BIN" --help >/dev/null 2>&1
if [ $? -eq 0 ]; then
  echo "  PASS: tela --help exits cleanly"
  ((PASSED++))
else
  echo "  FAIL: tela --help failed"
  ((FAILED++))
fi

tela_summary "test_build"
