#!/usr/bin/env bash
# Compare this port's CLI text (--help, usage errors, [ERROR] lines) with the
# upstream `sqlformat` console script, byte for byte. Upstream 0.6.1 fixes
# prog='sqlformat', so no rename is needed — unlike xlsx2csv, whose argparse
# uses argv[0].
#
# Usage: tools/cli_matrix.sh [path-to-port-binary]
# Requires: python3 with sqlparse installed (the baseline environment).
set -u

PORT=${1:-target/release/sqlformat-rs}
if [ ! -x "$PORT" ]; then
    echo "cli_matrix: $PORT is missing; run cargo build --release" >&2
    exit 2
fi
PORT=$(cd "$(dirname "$PORT")" && pwd)/$(basename "$PORT")

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
printf 'select a,b from t where x=1;\n' > "$tmp/kw.sql"
printf 'select 1;\n' > "$tmp/s1.sql"
cd "$tmp" || exit 2

fails=0
cases=0
gaps=0
run() {
    cases=$((cases + 1))
    python3 -m sqlparse "$@" > u.out 2> u.err
    urc=$?
    "$PORT" "$@" > r.out 2> r.err
    rrc=$?
    if [ "$urc" != "$rrc" ] || ! cmp -s u.out r.out || ! cmp -s u.err r.err; then
        fails=$((fails + 1))
        printf 'DIFF  %s\n' "$*"
        printf '  rc      %s / %s\n' "$urc" "$rrc"
        printf '  stdout  %s / %s bytes\n' "$(wc -c < u.out)" "$(wc -c < r.out)"
        printf '  stderr  %s / %s bytes\n' "$(wc -c < u.err)" "$(wc -c < r.err)"
        diff -u u.err r.err | head -12 | sed 's/^/  /'
    fi
}

# A flag upstream has and this port refuses on purpose. The case must DIFF;
# if it ever matches, the gap closed and KNOWN-GAPS.md is out of date.
gap() {
    gaps=$((gaps + 1))
    python3 -m sqlparse "$@" > u.out 2> u.err
    urc=$?
    "$PORT" "$@" > r.out 2> r.err
    rrc=$?
    if [ "$urc" = "$rrc" ] && cmp -s u.out r.out && cmp -s u.err r.err; then
        fails=$((fails + 1))
        printf 'GAP CLOSED but still listed: %s\n' "$*"
    fi
}

run
run --help
run --version
run s1.sql
run kw.sql -k upper
run kw.sql -i lower
run -r s1.sql
run --indent_width 4 s1.sql
run --wrap_after 20 s1.sql
run --comma_first true s1.sql
run --compact true s1.sql
# argparse shapes: unexpected, missing, invalid, attached and `=` forms
run --bogus s1.sql
run --bogus
run --bogus --x2 s1.sql
run -k bogus s1.sql
run -k=bad s1.sql
run -k=upper kw.sql
run -kupper kw.sql
run --keywords=upper kw.sql
run -o
run -o /tmp s1.sql
run --indent_width x s1.sql
run s1.sql s1.sql
run -o /tmp/out.csv s1.sql s1.sql
run --in-place -
run nope.sql
gap --encoding latin-1 s1.sql
gap -l python s1.sql
gap --indent_after_first s1.sql
gap --indent_columns s1.sql
gap --strip-comments s1.sql
gap -s s1.sql
gap -a s1.sql

if [ "$fails" -eq 0 ]; then
    echo "cli: $cases/$cases cases match upstream byte for byte; $gaps known gaps still refused"
    exit 0
fi
echo "cli: $fails unexpected result(s) over $cases matched cases + $gaps gaps" >&2
exit 1
