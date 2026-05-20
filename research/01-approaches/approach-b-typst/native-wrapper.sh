#!/usr/bin/env bash
# Adapter: office2pdf CLI uses `office2pdf -o OUT IN`; harness expects `cli IN OUT`.
set -e
exec office2pdf -o "$2" "$1"
