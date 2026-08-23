#!/bin/sh
# Runs the compat corpus with the release binary and prints the report.
set -e
cd "$(dirname "$0")/../.."
BAP_BIN="${BAP_BIN:-target/release/bap}" node bench/compat/run.mjs
"$BAP_BIN" report compat
