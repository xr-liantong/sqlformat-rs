#!/bin/sh
# Create the .venv the two Python cross-checks need (sqlparse).
#
# --system-site-packages so an sqlparse that is already installed for user
# python3 (pip install --user sqlparse) is visible without network access;
# if it is not there, fall back to pip.
set -eu
cd "$(dirname "$0")/.."

if [ -x .venv/bin/python ]; then
    echo ".venv already exists"
else
    python3 -m venv --system-site-packages .venv
    echo "created .venv"
fi

if .venv/bin/python -c 'import sqlparse' >/dev/null 2>&1; then
    echo "sqlparse $(.venv/bin/python -c 'import sqlparse; print(sqlparse.__version__)') is importable"
else
    .venv/bin/pip install --quiet sqlparse
    echo "installed sqlparse"
fi
