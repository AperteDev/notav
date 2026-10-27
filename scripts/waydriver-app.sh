#!/bin/bash
# The sandbox copy of the app (as `make -C linux sandbox`) for a waydriver-mcp session: give its
# path as start_session's command, with app_name "notav-waydriver". waydriver runs the app on the
# host's session bus (the keyring works) in a headless Mutter of its own; the sandbox copy is not
# unique there, so it runs beside the normal app, with its own data in target/sandbox.
#
# waydriver finds the app in the accessibility tree by name, on the host's bus: under its own
# name ("notav-waydriver", from argv[0]) a copy on the desktop is never taken for it.
# Build it first (`cargo build -p notav-linux`).
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
export NOTAV_SANDBOX=1 XDG_DATA_HOME="$root/target/sandbox"
exec -a notav-waydriver "$root/target/debug/notav-linux" "$@"
