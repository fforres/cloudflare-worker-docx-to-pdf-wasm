#!/usr/bin/env bash
# Measure compressed and uncompressed WASM size for a given .wasm file.
# Applies wasm-opt -Oz if not already optimized, then gzip -9.
#
# Usage: wasm-size.sh <wasm-file>

set -euo pipefail

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <wasm-file>" >&2
  exit 2
fi

WASM="$1"
if [[ ! -f "$WASM" ]]; then
  echo "not found: $WASM" >&2
  exit 2
fi

RAW=$(wc -c < "$WASM")

OPTIMIZED="${WASM%.wasm}.opt.wasm"
if command -v wasm-opt >/dev/null 2>&1; then
  wasm-opt -Oz \
    --enable-bulk-memory \
    --enable-sign-ext \
    --enable-nontrapping-float-to-int \
    --enable-mutable-globals \
    --enable-reference-types \
    --enable-multivalue \
    "$WASM" -o "$OPTIMIZED" 2>&1 >/tmp/wasm-opt.log \
    || { echo "  wasm-opt failed; see /tmp/wasm-opt.log; using unoptimized"; cp "$WASM" "$OPTIMIZED"; }
else
  cp "$WASM" "$OPTIMIZED"
fi
OPT=$(wc -c < "$OPTIMIZED")
GZ=$(gzip -9 -c "$OPTIMIZED" | wc -c)
BR=""
if command -v brotli >/dev/null 2>&1; then
  BR=$(brotli -q 11 -c "$OPTIMIZED" | wc -c)
fi

CF_LIMIT=$((10 * 1024 * 1024))

echo "WASM: $WASM"
printf "  raw         : %10d bytes (%.2f MiB)\n" "$RAW" "$(echo "scale=2; $RAW/1048576" | bc)"
printf "  wasm-opt -Oz: %10d bytes (%.2f MiB)\n" "$OPT" "$(echo "scale=2; $OPT/1048576" | bc)"
printf "  gzip -9     : %10d bytes (%.2f MiB)\n" "$GZ" "$(echo "scale=2; $GZ/1048576" | bc)"
if [[ -n "$BR" ]]; then
  printf "  brotli -q11 : %10d bytes (%.2f MiB)\n" "$BR" "$(echo "scale=2; $BR/1048576" | bc)"
fi
printf "  CF limit    : %10d bytes (10.00 MiB compressed)\n" "$CF_LIMIT"
if (( GZ <= CF_LIMIT )); then
  printf "  verdict     : ✓ FITS under 10 MiB compressed (margin %d bytes)\n" "$((CF_LIMIT - GZ))"
else
  printf "  verdict     : ✗ OVER 10 MiB compressed by %d bytes\n" "$((GZ - CF_LIMIT))"
fi
