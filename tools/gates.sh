#!/bin/sh
# Every mechanical rule for this project, in one command.
#
# Each line is a gate from docs/QUALITY-GATES.md; the exit code of the whole
# script is non-zero if any of them fails. Human judgement lives in the
# review, never in a rule that a machine can check.
#
# Issue #10: a gate that is green for the wrong reason is worse than no gate.
# tools/parity.sh now fixes EXPECTED_CASES, and the reverse tests below make it
# fail on purpose -- a dropped case, a corrupted golden, a missing fixture and a
# tampered total must each turn it red for that reason, not with a crash.
set -u
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"
# A stale CARGO_TARGET_DIR from another checkout makes `cargo run` reuse an old
# binary and the parity gate compares the wrong program (issue #10, false red).
unset CARGO_TARGET_DIR

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

tmp=$(mktemp -d)
trap 'rm -rf "$tmp" tools/parity-dropped.bash tools/parity-tampered.sh' EXIT

# expect_red <label> <stderr/stdout keyword> <command...>
expect_red() {
    label=$1
    keyword=$2
    shift 2
    if "$@" >"$tmp/out" 2>&1; then
        echo "FAIL reverse test: $label -- the gate stayed green"
        fail=$((fail + 1))
        return
    fi
    if ! grep -q -- "$keyword" "$tmp/out"; then
        echo "FAIL reverse test: $label -- red for the wrong reason (no '$keyword')"
        tail -5 "$tmp/out" | sed 's/^/     /'
        fail=$((fail + 1))
        return
    fi
    echo "PASS reverse test: $label"
    pass=$((pass + 1))
}

# 1. A case that is quietly dropped must fall foul of the fixed total.
sed 's/^check default golden-default.sql$/# &/' tools/parity.sh > tools/parity-dropped.sh
expect_red "one case commented out" "expected 24" bash tools/parity-dropped.sh

# 2. A corrupted golden must fail on the bytes.
cp fixtures/golden-r.sql "$tmp/golden-r.sql"
printf 'x' >> fixtures/golden-r.sql
expect_red "corrupted golden" "FAIL reindent" bash tools/parity.sh
cp "$tmp/golden-r.sql" fixtures/golden-r.sql

# 3. A missing fixture is a setup error, not a pass.
mv fixtures/kwcase.sql "$tmp/kwcase.sql"
expect_red "missing fixture" "missing fixture: fixtures/kwcase.sql" bash tools/parity.sh
mv "$tmp/kwcase.sql" fixtures/kwcase.sql

# 4. Tampering with the total itself must fail.
sed 's/^EXPECTED_CASES=24$/EXPECTED_CASES=25/' tools/parity.sh > tools/parity-tampered.sh
expect_red "tampered EXPECTED_CASES" "expected 25" bash tools/parity-tampered.sh

echo "gates: $pass passed, $fail failed"
[ "$fail" -eq 0 ]
