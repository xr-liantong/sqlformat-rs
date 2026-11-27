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
# latin-1 bytes on disk, and a BOM-prefixed UTF-16 file.
printf "select 'caf\xe9';\n" > "$tmp/lat.sql"
printf 'select 1; -- c\n' > "$tmp/c.sql"
printf 'select f(/*c*/x, y /*d*/) from t;\n' > "$tmp/par.sql"
printf 'select a+1, a*-1, a*(b+c), count(*), t.* from t;\n' > "$tmp/op.sql"
python3 -c "open('$tmp/u16.sql','wb').write(\"select 'caf\xe9';\n\".encode('utf-16'))"
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
run --encoding latin-1 lat.sql
run --encoding utf-16 u16.sql
run --encoding latin-1 s1.sql

# A decode failure is a traceback upstream: the class line still matches.
cases=$((cases + 1))
python3 -m sqlparse --encoding utf-8 lat.sql 2>&1 | tail -1 > u.err
"$PORT" --encoding utf-8 lat.sql 2>&1 | tail -1 > r.err
if ! cmp -s u.err r.err; then
    fails=$((fails + 1))
    printf 'DIFF  decode-error last line\n'
    diff -u u.err r.err | head -8 | sed 's/^/  /'
fi

# File output is written with --encoding, stdout is not.
cases=$((cases + 1))
rm -f u.lat r.lat
python3 -m sqlparse --encoding latin-1 lat.sql -o u.lat
"$PORT" --encoding latin-1 lat.sql -o r.lat
if ! cmp -s u.lat r.lat; then
    fails=$((fails + 1))
    printf 'DIFF  --encoding latin-1 -o (file bytes)\n'
fi
gap --encoding nope lat.sql
gap -l python s1.sql
run --strip-comments s1.sql
run --strip-comments c.sql
run --strip-comments -r c.sql
run -s op.sql
run --use_space_around_operators op.sql
run -s -r op.sql
run --indent_columns op.sql
run -r --indent_after_first op.sql
run --indent_columns --indent_after_first op.sql
# A comment directly before `)` survives as one space upstream; the port
# drops it (see docs/KNOWN-GAPS.md).
gap --strip-comments -r par.sql
gap -a s1.sql

if [ "$fails" -eq 0 ]; then
    echo "cli: $cases/$cases cases match upstream byte for byte; $gaps known gaps still refused"
    exit 0
fi
echo "cli: $fails unexpected result(s) over $cases matched cases + $gaps gaps" >&2
exit 1
