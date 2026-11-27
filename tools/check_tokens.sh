#!/bin/sh
# Compare the Rust lexer's token stream with sqlparse's, token by token.
#
# The lexer applies SQL_REGEX in order, so this catches regex order, the
# hand-rewritten lookarounds, and Python-vs-Rust \w differences at once.
# Usage: tools/check_tokens.sh [file ...]   (default: the three fixtures)
set -eu
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

[ $# -gt 0 ] || set -- fixtures/tokens.sql fixtures/kwcase.sql fixtures/corpus-20k.sql

if [ ! -x .venv/bin/python ]; then
    echo "FAIL tokens: .venv/bin/python is missing; run tools/setup_venv.sh"
    exit 2
fi

cargo build --release --bin txdump >/dev/null 2>&1

failed=0
checked=0
for f in "$@"; do
    if [ ! -f "$f" ]; then
        echo "SKIP $f (missing)"
        continue
    fi
    .venv/bin/python tools/py_tokens.py "$f" > /tmp/py.tokens
    ./target/release/txdump "$f" > /tmp/rs.tokens
    n=$(wc -l < /tmp/py.tokens)
    if cmp -s /tmp/py.tokens /tmp/rs.tokens; then
        echo "PASS $f ($n tokens)"
        checked=$((checked + 1))
    else
        echo "FAIL $f: first difference:"
        cmp /tmp/py.tokens /tmp/rs.tokens | head -2
        failed=$((failed + 1))
    fi
done
echo "tokens: $checked passed, $failed failed"
[ "$failed" -eq 0 ]
