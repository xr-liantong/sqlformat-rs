#!/bin/sh
# Every mechanical rule for this project, in one command.
#
# Each line is a gate from docs/QUALITY-GATES.md; the exit code of the whole
# script is non-zero if any of them fails. Human judgement lives in the
# review, never in a rule that a machine can check.
set -u
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

pass=0
fail=0
run() {
    name=$1
    shift
    if "$@" > /tmp/gate.out 2>&1; then
        echo "PASS $name"
        pass=$((pass + 1))
    else
        echo "FAIL $name"
        tail -15 /tmp/gate.out | sed 's/^/     /'
        fail=$((fail + 1))
    fi
}

run "fmt"      cargo fmt --check
run "clippy"   cargo clippy --release --all-targets -- -D warnings
run "test"     cargo test --release
run "parity"   tools/parity.sh
run "keywords" .venv/bin/python tools/check_keywords.py
run "tokens"   tools/check_tokens.sh

echo "gates: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
