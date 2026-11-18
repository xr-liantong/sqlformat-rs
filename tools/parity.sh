#!/usr/bin/env bash
# Byte parity against the upstream golden files.
#
# Runs the locally built binary (override with SQLFORMAT_RS=/path/to/binary).
# Exits non-zero on the first mismatch, on a missing fixture, or on a wrong
# exit code — a green run always means the bytes were compared.
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ -n "${SQLFORMAT_RS:-}" ]]; then
    bin=("$SQLFORMAT_RS")
else
    cargo build --release --quiet --bin sqlformat-rs
    bin=(cargo run --release --quiet --bin sqlformat-rs --)
fi

pass=0
fail=0

check() {
    local name=$1 golden=$2
    shift 2
    local out
    out=$(mktemp)
    if ! "${bin[@]}" "$@" fixtures/kwcase.sql >"$out"; then
        echo "FAIL $name: non-zero exit"
        fail=$((fail + 1))
        rm -f "$out"
        return
    fi
    if cmp -s "$out" "fixtures/$golden"; then
        echo "ok   $name ($(stat -c%s "$out") B)"
        pass=$((pass + 1))
    else
        local got
        got=$(stat -c%s "$out")
        echo "FAIL $name: got $got B, want $(stat -c%s "fixtures/$golden") B"
        diff <(head -40 "$out") <(head -40 "fixtures/$golden") | head -20 || true
        fail=$((fail + 1))
    fi
    rm -f "$out"
}

check default golden-default.sql
check reindent golden-r.sql -r
check keywords-upper golden-kupper.sql -k upper
check keywords-lower golden-klower.sql -k lower
check identifiers-upper golden-iupper.sql -i upper
check indent-width-4 golden-iw4.sql --indent_width 4

# Unsupported flags must refuse, not guess.
if "${bin[@]}" -a fixtures/kwcase.sql >/dev/null 2>&1; then
    echo "FAIL -a: expected exit 2, got 0"
    fail=$((fail + 1))
else
    code=$?
    if [[ $code -eq 2 ]]; then
        echo "ok   -a exits 2"
        pass=$((pass + 1))
    else
        echo "FAIL -a: expected exit 2, got $code"
        fail=$((fail + 1))
    fi
fi

echo "$pass passed, $fail failed"
[[ $fail -eq 0 ]]
