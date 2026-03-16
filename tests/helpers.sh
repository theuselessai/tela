#!/usr/bin/env bash
# Tela TUI test helpers — tmux-based terminal testing
set -euo pipefail

TELA_BIN="${TELA_BIN:-./target/debug/tela-cli}"
TEST_COLS="${TEST_COLS:-80}"
TEST_ROWS="${TEST_ROWS:-24}"
PASSED=0
FAILED=0
ERRORS=""

# Start a tmux session running a tela app
# Usage: tela_start <session_name> <app_path>
tela_start() {
  local session="$1"
  local app_path="$2"

  # Kill any existing session with this name
  tmux kill-session -t "$session" 2>/dev/null || true

  # Create detached session with fixed size
  tmux new-session -d -s "$session" -x "$TEST_COLS" -y "$TEST_ROWS"

  # Run tela in the session
  tmux send-keys -t "$session" "$TELA_BIN run $app_path" Enter

  # Wait for app to render
  sleep 1
}

# Send keys to a tmux session
# Usage: tela_keys <session_name> <keys>
tela_keys() {
  local session="$1"
  shift
  tmux send-keys -t "$session" "$@"
  sleep 0.3
}

# Capture the current terminal content
# Usage: tela_capture <session_name>
tela_capture() {
  local session="$1"
  tmux capture-pane -t "$session" -p
}

# Assert that the terminal contains expected text
# Usage: tela_assert_contains <session_name> <expected_text> <test_description>
tela_assert_contains() {
  local session="$1"
  local expected="$2"
  local desc="${3:-contains '$expected'}"
  local content
  content=$(tela_capture "$session")

  if echo "$content" | grep -qF "$expected"; then
    echo "  PASS: $desc"
    ((PASSED++))
  else
    echo "  FAIL: $desc"
    echo "    expected to find: $expected"
    echo "    actual output:"
    echo "$content" | head -5 | sed 's/^/      /'
    ((FAILED++))
    ERRORS="${ERRORS}\n  - $desc"
  fi
}

# Assert that the terminal does NOT contain text
# Usage: tela_assert_not_contains <session_name> <unexpected_text> <test_description>
tela_assert_not_contains() {
  local session="$1"
  local unexpected="$2"
  local desc="${3:-does not contain '$unexpected'}"
  local content
  content=$(tela_capture "$session")

  if echo "$content" | grep -qF "$unexpected"; then
    echo "  FAIL: $desc"
    echo "    found unexpected: $unexpected"
    ((FAILED++))
    ERRORS="${ERRORS}\n  - $desc"
  else
    echo "  PASS: $desc"
    ((PASSED++))
  fi
}

# Assert screen content matches a regex
# Usage: tela_assert_matches <session_name> <regex> <test_description>
tela_assert_matches() {
  local session="$1"
  local pattern="$2"
  local desc="${3:-matches '$pattern'}"
  local content
  content=$(tela_capture "$session")

  if echo "$content" | grep -qE "$pattern"; then
    echo "  PASS: $desc"
    ((PASSED++))
  else
    echo "  FAIL: $desc"
    echo "    expected to match: $pattern"
    ((FAILED++))
    ERRORS="${ERRORS}\n  - $desc"
  fi
}

# Stop a tmux session
# Usage: tela_stop <session_name>
tela_stop() {
  local session="$1"
  # Try quitting gracefully first
  tmux send-keys -t "$session" C-c 2>/dev/null || true
  sleep 0.3
  tmux kill-session -t "$session" 2>/dev/null || true
}

# Print test summary
# Usage: tela_summary <test_file_name>
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

# Build example bundles (requires esbuild or npx)
# Usage: tela_build_example <example_path>
tela_build_example() {
  local app_path="$1"
  local src="$app_path/src/index.jsx"
  local out="$app_path/bundle.js"

  if [ ! -f "$out" ]; then
    if command -v esbuild &>/dev/null; then
      esbuild "$src" --bundle --jsx-factory=h --jsx-fragment=Fragment --outfile="$out" 2>/dev/null
    elif command -v npx &>/dev/null; then
      npx esbuild "$src" --bundle --jsx-factory=h --jsx-fragment=Fragment --outfile="$out" 2>/dev/null
    else
      echo "ERROR: esbuild not found. Install with: npm install -g esbuild"
      return 1
    fi
  fi
}
