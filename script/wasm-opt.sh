#!/usr/bin/env sh
# wasm-opt.sh - post-process the trunk WASM output with binaryen for
# maximum size reduction.
#
# Why this exists: wasm-bindgen emits bulk-memory (memory.copy/fill) and
# nontrapping-float-to-int instructions into the wasm. The wasm-opt binary
# rejects those unless the matching --enable-* features are passed, and
# trunk does not pass them - so trunk's built-in wasm-opt step silently
# fails on modern rustc output. We disable trunk's step (data-wasm-opt="0"
# in frontend/index.html) and run wasm-opt ourselves here with the
# features enabled.
#
# Usage:
#   sh script/wasm-opt.sh          # from the repo root (tauri beforeBuildCommand runs it)
#
# Requires: binaryen (wasm-opt) on PATH.

set -eu

cd "$(dirname "$0")/.."

DIST="apps/med-recon-app/frontend/dist"

WASM=$(ls "$DIST"/*_bg.wasm 2>/dev/null | head -1 || true)
if [ -z "$WASM" ]; then
    echo "wasm-opt.sh: no *_bg.wasm found in $DIST, nothing to do" >&2
    exit 0
fi

echo "wasm-opt.sh: optimizing $WASM"
wasm-opt \
    -Oz \
    --strip-debug \
    --low-memory-unused \
    --enable-bulk-memory-opt \
    --enable-nontrapping-float-to-int \
    --enable-mutable-globals \
    -o "$WASM" \
    "$WASM"
echo "wasm-opt.sh: done"