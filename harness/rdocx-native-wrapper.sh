#!/usr/bin/env bash
# Adapter: rdocx CLI uses `convert -t pdf -o OUT IN`; our harness expects `cli IN OUT`.
set -e
exec rdocx convert -t pdf -o "$2" "$1"
