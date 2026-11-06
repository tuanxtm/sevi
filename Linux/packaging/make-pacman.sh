#!/bin/sh
# Compatibility entrypoint; implementation lives in the Rust workspace.
set -eu
cd "$(dirname "$0")/../.."
exec cargo xtask tool make-pacman "$@"
