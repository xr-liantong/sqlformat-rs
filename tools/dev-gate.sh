#!/bin/sh
# Everything a working tree has to pass, in one command.
#
#   tools/dev-gate.sh            # normal use
#   tools/dev-gate.sh --no-venv  # do not touch .venv, just run the gates
#
# This is the entry point for people. `tools/gates.sh` stays the entry point for
# machines: it never installs anything, never touches the network, and reports a
# missing dependency as a failure with the command that fixes it (issue #10).
# Bootstrapping here instead of there keeps both properties: one command for a
# developer, and a verdict in the gate that depends on the code alone.
set -eu
cd "$(dirname "$0")/.."

if [ "${1:-}" = "--no-venv" ]; then
    shift
else
    tools/setup_venv.sh
fi

exec bash tools/gates.sh "$@"
