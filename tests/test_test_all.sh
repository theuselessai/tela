#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "$0")/.." && pwd)"
source "$DIR/tests/helpers.sh"

echo "=== test_test_all ==="

tela_build_example "$DIR/examples/test-all"
tela_start "test-all" "$DIR/examples/test-all"

tela_assert_contains "test-all" "Test All" "renders title"
tela_assert_contains "test-all" "normal" "shows normal mode"
tela_assert_contains "test-all" "List" "renders List tab"
tela_assert_contains "test-all" "Counter" "renders Counter tab label"
tela_assert_contains "test-all" "Table" "renders Table tab label"

tela_assert_contains "test-all" "Item A" "renders list items"
tela_assert_contains "test-all" "[ ]" "shows unchecked items"
tela_assert_contains "test-all" "[x]" "shows checked items"

tela_keys "test-all" "j"
tela_keys "test-all" " "
tela_assert_not_contains "test-all" "[x] Item B" "toggle unchecks Item B"

tela_keys "test-all" "t"
tela_assert_contains "test-all" "Count:" "tab switches to counter view"

tela_keys "test-all" "+"
tela_keys "test-all" "+"
tela_assert_contains "test-all" "Count: 2" "increment works on counter tab"

tela_keys "test-all" "-"
tela_assert_contains "test-all" "Count: 1" "decrement works on counter tab"

tela_keys "test-all" "t"
tela_assert_contains "test-all" "#" "tab switches to table view"
tela_assert_contains "test-all" "Item" "table shows header"
tela_assert_contains "test-all" "Pending" "table shows status"

tela_keys "test-all" "t"
tela_assert_contains "test-all" "Item A" "tab cycles back to list"

tela_keys "test-all" "i"
tela_assert_contains "test-all" "insert" "i enters insert mode"

tela_keys "test-all" "N" "e" "w"
tela_assert_contains "test-all" "New" "typing appears in input"

tela_keys "test-all" "Enter"
tela_assert_contains "test-all" "New" "submitted item appears in list"
tela_assert_contains "test-all" "normal" "submit returns to normal mode"

tela_stop "test-all"
tela_summary "test_test_all"
