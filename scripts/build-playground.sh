#!/usr/bin/env bash
# The playground as a static site: build/rangerthree-core.js, the facade
# and playground/ into build/site (what the GitHub Pages workflow deploys).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
bash "$ROOT/scripts/build-js.sh"
SITE="$ROOT/build/site"
rm -rf "$SITE"; mkdir -p "$SITE"
cp "$ROOT/build/rangerthree-core.js" "$ROOT"/js/*.js "$ROOT"/playground/* "$SITE/"
echo "build/site ($(ls "$SITE" | wc -l) files)"
