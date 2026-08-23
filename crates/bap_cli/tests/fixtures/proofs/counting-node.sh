#!/bin/sh
# Test shim: records one line per launch, then runs the real Node.
printf 'x\n' >> "$BAP_PROOF_NODE_LOG"
exec node "$@"
