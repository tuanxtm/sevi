#!/bin/sh
# Existing entrypoint; all engine checks and generated binaries now run through Cargo.
set -eu
cd "$(dirname "$0")/../.."
exec cargo xtask check "$@"
