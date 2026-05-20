#!/usr/bin/env bash
# Build the docx-to-pdf-wasm package.
#
# Compiles the TypeScript sources in src/ into build/ via tsc, and copies the
# WASM artifact from the research opt-8 build into build/docx-to-pdf.wasm.
#
# Prereqs:
#   - pnpm install (at the repo root) — provides tsc via the workspace.
#   - To rebuild the WASM from Rust source, run scripts/build-wasm.sh first.
#     This requires Rust + wasm32-unknown-unknown + Python (fontTools).
set -euo pipefail

PKG_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO_ROOT="$(cd "$PKG_ROOT/../.." && pwd)"

cd "$PKG_ROOT"
echo "→ compiling TypeScript"
if command -v pnpm >/dev/null 2>&1; then
  pnpm exec tsc
else
  npx tsc
fi

WASM_SRC="$REPO_ROOT/research/02-optimizations/opt-8-textbox-preprocessor-subset/converter/target/wasm32-unknown-unknown/release/approach_c_rdocx_opt8.wasm"

if [[ -f "$WASM_SRC" ]]; then
  echo "→ optimising and copying WASM (from opt-8 build)"
  if command -v wasm-opt >/dev/null 2>&1; then
    wasm-opt -Oz \
      --enable-bulk-memory \
      --enable-sign-ext \
      --enable-nontrapping-float-to-int \
      --enable-mutable-globals \
      --enable-reference-types \
      --enable-multivalue \
      "$WASM_SRC" \
      -o "$PKG_ROOT/build/docx-to-pdf.wasm"
  else
    echo "  (wasm-opt not found, copying unoptimised)"
    cp "$WASM_SRC" "$PKG_ROOT/build/docx-to-pdf.wasm"
  fi
else
  echo "  ! WASM not found at $WASM_SRC"
  echo "  ! run scripts/build-wasm.sh first, or use the pre-built blob already in build/"
fi

echo "→ build artifacts in $PKG_ROOT/build:"
ls -la "$PKG_ROOT/build"
