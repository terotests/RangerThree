#!/usr/bin/env bash
# The three crate (three/src/lib.rs) as Kotlin, written by Ranger into
# <dir>/rangerthree.kt, package rangerthree.core.
#
#   bash gpu-android/gen-kotlin.sh <dir>
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$(mkdir -p "${1:?usage: gen-kotlin.sh <dir>}" && cd "$1" && pwd)"
# shellcheck source=../scripts/deps.sh
source "$ROOT/scripts/deps.sh"
ensure_ranger
LOG="$OUT/rangerthree.log"
rm -f "$OUT/rangerthree.kt"
(cd "$ROOT/three" && node "$RANGER/dist/rgrc.js" -l=kotlin -ktpackage=rangerthree.core src/lib.rs -d="$OUT" -o=rangerthree.kt -no-color) >"$LOG" 2>&1 || true
if grep -q '\[FAIL' "$LOG" || [[ ! -s "$OUT/rangerthree.kt" ]]; then
  grep -E '\[FAIL' -A3 "$LOG" | head -40
  echo "gen-kotlin: Ranger did not write three/src/lib.rs as Kotlin (log: $LOG)" >&2
  exit 1
fi
rm -f "$LOG"
