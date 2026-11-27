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

# The number of cases this script must execute. Issue #10: commenting a case
# out used to leave the gate green, so the total is checked at the end too.
EXPECTED_CASES=27

check() {
    local name=$1 golden=$2
    shift 2
    if [[ ! -f fixtures/kwcase.sql ]]; then
        echo "FAIL $name: missing fixture: fixtures/kwcase.sql"
        fail=$((fail + 1))
        return
    fi
    if [[ ! -f "fixtures/$golden" ]]; then
        echo "FAIL $name: missing golden: fixtures/$golden"
        fail=$((fail + 1))
        return
    fi
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

check_in() {
    local name=$1 golden=$2 input=$3
    shift 3
    if [[ ! -f "$input" ]]; then
        echo "FAIL $name: missing fixture: $input"
        fail=$((fail + 1))
        return
    fi
    if [[ ! -f "fixtures/$golden" ]]; then
        echo "FAIL $name: missing golden: fixtures/$golden"
        fail=$((fail + 1))
        return
    fi
    local out
    out=$(mktemp)
    if ! "${bin[@]}" "$@" "$input" >"$out"; then
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

check_in strip-comments golden-strip.sql fixtures/comments.sql --strip-comments
check_in strip-comments-reindent golden-strip-r.sql fixtures/comments.sql --strip-comments -r

# --encoding: the fixture is latin-1 on disk, the golden is what upstream writes
# to stdout (always the process encoding, not --encoding).
check_in encoding-latin1 golden-latin1.sql fixtures/latin1.sql --encoding latin-1
check_in space-operators golden-operators.sql fixtures/operators.sql -s
check_in space-operators-reindent golden-operators-r.sql fixtures/operators.sql -s -r
check_in indent-columns golden-indent-columns.sql fixtures/indent.sql --indent_columns
check_in indent-after-first golden-indent-after-first.sql fixtures/indent.sql -r --indent_after_first
check_in indent-both golden-indent-both.sql fixtures/indent.sql -r --indent_columns --indent_after_first

# v0.2 reindent options, compared on a list/CASE heavy fixture.
# (This line used to be glued to the comment above, so the case never ran --
# issue #10: a case that silently stops running used to leave the gate green.)
check_in wrap_after-20 golden-wrap20.sql fixtures/lists.sql -r --wrap_after 20
check_in comma_first golden-comma.sql fixtures/lists.sql -r --comma_first true
check_in comma_first-wrap_after-30 golden-comma-w30.sql fixtures/lists.sql -r --comma_first true --wrap_after 30
check_in compact golden-compact.sql fixtures/lists.sql -r --compact true
check_in all-three golden-all3.sql fixtures/lists.sql -r --wrap_after 22 --comma_first true --compact true

# Name.Builtin is not an identifier upstream: no IdentifierList is built from
# `count(*), TIMESTAMP, MAP, CHARACTER`, so nothing gets split per column.
check_in comma-multiline golden-comma-multiline.sql fixtures/comma-multiline.sql -r --comma_first true
check_in plain-multiline golden-plain-multiline.sql fixtures/comma-multiline.sql -r
check_in builtin-names golden-ddl.sql fixtures/ddl-lists.sql -r

# Issue #5: a group extended after creation must grow its cached length like
# upstream `grp.value += ...`, otherwise wrap_after mis-places the break in CTE
# alias lists and non-ASCII identifier lists. Baseline 72a0ab9: 703/646/228 B.
check_in cte-wrap20 golden-cte-w20.sql fixtures/cte-list.sql -r --wrap_after 20
check_in cte-wrap40 golden-cte-w40.sql fixtures/cte-list.sql -r --wrap_after 40
check_in unicode-wrap20 golden-unicode-w20.sql fixtures/unicode-list.sql -r --wrap_after 20

# Limits: more than 10_000 tokens in one list is refused by upstream with
# exit 1, empty stdout and one stderr line. Compare all three.
limit_out=$(mktemp)
limit_err=$(mktemp)
if "${bin[@]}" -r fixtures/tokens-limit.sql >"$limit_out" 2>"$limit_err"; then
    echo "FAIL token-limit: expected exit 1, got 0"
    fail=$((fail + 1))
elif [[ -s "$limit_out" ]]; then
    echo "FAIL token-limit: stdout must stay empty"
    fail=$((fail + 1))
elif grep -qx "\[ERROR\] Failed to format fixtures/tokens-limit.sql: Maximum number of tokens exceeded (10000)." "$limit_err"; then
    echo "ok   token-limit (exit 1, empty stdout, one stderr line)"
    pass=$((pass + 1))
else
    echo "FAIL token-limit: unexpected stderr"
    head -3 "$limit_err"
    fail=$((fail + 1))
fi
rm -f "$limit_out" "$limit_err"

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

handled=$((pass + fail))
if [[ $handled -ne $EXPECTED_CASES ]]; then
    echo "FAIL total: $handled cases handled, expected $EXPECTED_CASES"
    exit 2
fi

echo "$pass passed, $fail failed"
[[ $fail -eq 0 ]]
