#!/bin/sh
# Same engine lock on every desktop; build products stay under target/.
set -eu
cd "$(dirname "$0")/.."
exec cargo xtask check "$@"
