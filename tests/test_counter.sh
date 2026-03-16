#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "$0")/.." && pwd)"
source "$DIR/tests/helpers.sh"

echo "=== test_counter ==="

tela_build_example "$DIR/examples/counter"
tela_start "test-counter" "$DIR/examples/counter"

tela_assert_contains "test-counter" "Counter" "renders title"
tela_assert_contains "test-counter" "Count:" "renders count display"
tela_assert_contains "test-counter" "0" "initial count is 0"

tela_keys "test-counter" "j"
tela_assert_contains "test-counter" "1" "j increments count to 1"

tela_keys "test-counter" "j"
tela_assert_contains "test-counter" "2" "j increments count to 2"

tela_keys "test-counter" "k"
tela_assert_contains "test-counter" "1" "k decrements count to 1"

tela_stop "test-counter"
tela_summary "test_counter"
