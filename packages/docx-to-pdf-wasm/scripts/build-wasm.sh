#!/usr/bin/env bash
# Build the WASM artifact from Rust source.
#
# Prereqs:
#   - Rust 1.95+ with the wasm32-unknown-unknown target installed
#       rustup target add wasm32-unknown-unknown
#   - Python 3 with fontTools + brotli (used by build.rs to subset fonts)
#       pip3 install --user fontTools brotli
#   - wasm-opt (from binaryen) optional but recommended:
#       brew install binaryen   (or your distro equivalent)
set -euo pipefail

PKG_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$PKG_ROOT/../.." && pwd)"
CRATE_DIR="$REPO_ROOT/research/02-optimizations/opt-8-textbox-preprocessor-subset/converter"

if [[ ! -d "$CRATE_DIR" ]]; then
  echo "Rust crate not found at $CRATE_DIR" >&2
  exit 1
fi

cd "$CRATE_DIR"
echo "→ cargo build --release --target wasm32-unknown-unknown"
cargo build --release --target wasm32-unknown-unknown --no-default-features

echo "→ optimise + copy"
WASM_OUT="$CRATE_DIR/target/wasm32-unknown-unknown/release/approach_c_rdocx_opt8.wasm"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz \
    --enable-bulk-memory \
    --enable-sign-ext \
    --enable-nontrapping-float-to-int \
    --enable-mutable-globals \
    --enable-reference-types \
    --enable-multivalue \
    "$WASM_OUT" -o "$PKG_ROOT/build/docx-to-pdf.wasm"
else
  cp "$WASM_OUT" "$PKG_ROOT/build/docx-to-pdf.wasm"
fi

ls -la "$PKG_ROOT/build/docx-to-pdf.wasm"
