#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
# The reference three.js (and React Three Fiber) the parity goldens come
# from, into tmp/three-ref (git-ignored; test reference only, never shipped
# or imported by M-Files). Pin = Ranger's game_engine/three reference pin.
set -euo pipefail
THREE_VERSION="0.185.1"
R3F_VERSION="9.8.2"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DEST="$ROOT/tmp/three-ref"
[[ -f "$DEST/three/build/three.module.js" && -f "$DEST/.v-$THREE_VERSION" ]] && { echo "three@$THREE_VERSION in $DEST"; exit 0; }
rm -rf "$DEST"; mkdir -p "$DEST"; cd "$DEST"
npm pack "three@$THREE_VERSION" "@react-three/fiber@$R3F_VERSION" >/dev/null 2>&1
tar xzf "three-$THREE_VERSION.tgz" && mv package three
tar xzf "react-three-fiber-$R3F_VERSION.tgz" && mv package r3f
rm -f ./*.tgz; touch ".v-$THREE_VERSION"
echo "three@$THREE_VERSION, @react-three/fiber@$R3F_VERSION in $DEST"
