#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TELA_BIN="${TELA_BIN:-$SCRIPT_DIR/target/debug/tela-cli}"
TEST_COLS="${TEST_COLS:-80}"
TEST_ROWS="${TEST_ROWS:-24}"
STARTUP_WAIT="${STARTUP_WAIT:-2}"
PASSED=0
FAILED=0
ERRORS=""

pass() { PASSED=$((PASSED + 1)); }
fail() { FAILED=$((FAILED + 1)); }

tela_start() {
  local session="$1"
  local app_path="$2"

  tmux kill-session -t "$session" 2>/dev/null || true
  tmux new-session -d -s "$session" -x "$TEST_COLS" -y "$TEST_ROWS"
  tmux send-keys -t "$session" "$TELA_BIN run $app_path 2>/tmp/tela-test-$session.log" Enter
  sleep "$STARTUP_WAIT"

  local content
  content=$(tmux capture-pane -t "$session" -p)
  if [ -z "$(echo "$content" | tr -d '[:space:]')" ]; then
    echo "  WARNING: screen is blank after startup"
    if [ -f "/tmp/tela-test-$session.log" ]; then
      echo "  stderr:" && head -5 "/tmp/tela-test-$session.log" | sed 's/^/    /'
    fi
  fi
}

tela_keys() {
  local session="$1"
  shift
  tmux send-keys -t "$session" "$@"
  sleep 0.3
}

tela_capture() {
  local session="$1"
  tmux capture-pane -t "$session" -p
}

tela_assert_contains() {
  local session="$1"
  local expected="$2"
  local desc="${3:-contains '$expected'}"
  local content
  content=$(tela_capture "$session")

  if echo "$content" | grep -qF "$expected"; then
    echo "  PASS: $desc"
    pass
  else
    echo "  FAIL: $desc"
    echo "    expected to find: $expected"
    echo "    actual output:"
    echo "$content" | head -5 | sed 's/^/      /'
    fail
    ERRORS="${ERRORS}\n  - $desc"
  fi
}

tela_assert_not_contains() {
  local session="$1"
  local unexpected="$2"
  local desc="${3:-does not contain '$unexpected'}"
  local content
  content=$(tela_capture "$session")

  if echo "$content" | grep -qF "$unexpected"; then
    echo "  FAIL: $desc"
    echo "    found unexpected: $unexpected"
    fail
    ERRORS="${ERRORS}\n  - $desc"
  else
    echo "  PASS: $desc"
    pass
  fi
}

tela_assert_matches() {
  local session="$1"
  local pattern="$2"
  local desc="${3:-matches '$pattern'}"
  local content
  content=$(tela_capture "$session")

  if echo "$content" | grep -qE "$pattern"; then
    echo "  PASS: $desc"
    pass
  else
    echo "  FAIL: $desc"
    echo "    expected to match: $pattern"
    fail
    ERRORS="${ERRORS}\n  - $desc"
  fi
}

tela_stop() {
  local session="$1"
  tmux send-keys -t "$session" C-c 2>/dev/null || true
  sleep 0.3
  tmux kill-session -t "$session" 2>/dev/null || true
}

tela_summary() {
  local name="${1:-tests}"
  echo ""
  echo "[$name] $PASSED passed, $FAILED failed"
  if [ "$FAILED" -gt 0 ]; then
    echo -e "Failures:$ERRORS"
    return 1
  fi
  return 0
}

tela_build_example() {
  local app_path="$1"
  local src="$app_path/src/index.jsx"
  local out="$app_path/bundle.js"

  if [ ! -f "$out" ]; then
    if command -v esbuild &>/dev/null; then
      esbuild "$src" --jsx-factory=h --jsx-fragment=Fragment --outfile="$out"
    elif command -v npx &>/dev/null; then
      npx esbuild "$src" --jsx-factory=h --jsx-fragment=Fragment --outfile="$out"
    else
      echo "ERROR: esbuild not found. Install with: npm install -g esbuild"
      return 1
    fi
  fi
}
