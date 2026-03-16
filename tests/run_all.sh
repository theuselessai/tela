#!/usr/bin/env bash
set -uo pipefail
DIR="$(cd "$(dirname "$0")/.." && pwd)"

TOTAL_PASSED=0
TOTAL_FAILED=0
FAILED_SUITES=""

run_test() {
  local script="$1"
  local name
  name=$(basename "$script" .sh)

  echo ""
  if bash "$script"; then
    echo "  => $name: OK"
  else
    echo "  => $name: FAILED"
    FAILED_SUITES="${FAILED_SUITES}\n  - $name"
    ((TOTAL_FAILED++))
    return
  fi
  ((TOTAL_PASSED++))
}

echo "==============================="
echo " Tela Test Suite"
echo "==============================="

run_test "$DIR/tests/test_build.sh"
run_test "$DIR/tests/test_counter.sh"
run_test "$DIR/tests/test_todo.sh"
run_test "$DIR/tests/test_test_all.sh"

echo ""
echo "==============================="
echo " Results: $TOTAL_PASSED suites passed, $TOTAL_FAILED suites failed"
echo "==============================="

if [ "$TOTAL_FAILED" -gt 0 ]; then
  echo -e "Failed suites:$FAILED_SUITES"
  exit 1
fi

exit 0
