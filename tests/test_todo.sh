#!/usr/bin/env bash
set -euo pipefail
DIR="$(cd "$(dirname "$0")/.." && pwd)"
source "$DIR/tests/helpers.sh"

echo "=== test_todo ==="

tela_build_example "$DIR/examples/todo"
tela_start "test-todo" "$DIR/examples/todo"

tela_assert_contains "test-todo" "Todo List" "renders title"
tela_assert_contains "test-todo" "Learn Tela framework" "renders first todo item"
tela_assert_contains "test-todo" "Build a TUI app" "renders second todo item"
tela_assert_contains "test-todo" "Tasks" "renders tabs"

tela_keys "test-todo" "j"
sleep 0.2
tela_keys "test-todo" " "
tela_assert_contains "test-todo" "[x]" "space toggles item done"

tela_keys "test-todo" "t"
sleep 0.3
tela_assert_contains "test-todo" "Stats" "t switches to stats tab"
tela_assert_matches "test-todo" "[0-9]" "gauge shows numeric content"

tela_stop "test-todo"
tela_summary "test_todo"
