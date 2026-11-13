#!/usr/bin/env bash
# The Kotlin GPU player tested on the JVM, headless (Mesa EGL, apt libegl1
# libegl-mesa0 libgles2), without an Android device:
#
#   1. Ranger writes the three crate (three/src/lib.rs) as Kotlin
#      (gen-kotlin.sh, package rangerthree.core).
#   2. kotlinc compiles it with player/ (GpuPlayer, Gl), scenes/ and jvm/
#      (JNA GLES/EGL).
#   3. Scenes: the Kotlin crate's GpuRenderer + GpuPlayer draw two scenes,
#      compared with the Kotlin SoftRenderer.
#   4. Replay (when a dump dir is given): the streams tests/gpu-compare.mjs
#      --dump wrote, compared with Chromium's pixels.
#   5. The Android app's Kotlin (app/, android/AndroidGl.kt) compiled against
#      android-all (the Android API), when ANDROID_CHECK=1. The APK itself
#      needs the Android SDK: cd gpu-android/app && ./gradlew installDebug.
#
#   bash gpu-android/test.sh [tmp/gpu-dump]
#
# kotlinc, JNA and android-all are downloaded into tmp/kotlin once.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# shellcheck source=../scripts/deps.sh
source "$ROOT/scripts/deps.sh"
ensure_ranger
T="$ROOT/tmp/kotlin"
mkdir -p "$T"
KOTLIN_VERSION=2.1.20
JNA_VERSION=5.14.0
ANDROID_ALL=14-robolectric-10818077
if [[ ! -x "$T/kotlinc/bin/kotlinc" ]]; then
  echo "==> kotlinc $KOTLIN_VERSION" >&2
  curl -sSL -o "$T/kc.zip" "https://github.com/JetBrains/kotlin/releases/download/v$KOTLIN_VERSION/kotlin-compiler-$KOTLIN_VERSION.zip"
  (cd "$T" && unzip -q -o kc.zip && rm kc.zip)
fi
if [[ ! -s "$T/jna.jar" ]]; then
  curl -sSL -o "$T/jna.jar" "https://repo1.maven.org/maven2/net/java/dev/jna/jna/$JNA_VERSION/jna-$JNA_VERSION.jar"
fi
KOTLINC="$T/kotlinc/bin/kotlinc"
STDLIB="$T/kotlinc/lib/kotlin-stdlib.jar"
P="$ROOT/gpu-android"

echo "==> three crate -> Kotlin (Ranger)" >&2
bash "$P/gen-kotlin.sh" "$T"

echo "==> kotlinc (crate + player + JVM host)" >&2
rm -rf "$T/out"
"$KOTLINC" -nowarn "$P/player/src" "$P/scenes/src" "$P/jvm/src" "$T/rangerthree.kt" -cp "$T/jna.jar" -d "$T/out" 2>&1 | grep -v '^warning' || true
[[ -f "$T/out/rangerthree/gpu/ScenesKt.class" ]] || { echo "gpu-android: kotlinc failed" >&2; exit 1; }
CP="$T/out:$T/jna.jar:$STDLIB"

echo "==> Scenes (Kotlin crate + Kotlin player vs SoftRenderer)" >&2
(cd "$ROOT" && java -cp "$CP" rangerthree.gpu.ScenesKt)

if [[ $# -gt 0 ]]; then
  echo "==> Replay $1" >&2
  (cd "$ROOT" && java -cp "$CP" rangerthree.gpu.ReplayKt "$1")
fi

if [[ "${ANDROID_CHECK:-0}" == "1" ]]; then
  if [[ ! -s "$T/android-all.jar" ]]; then
    curl -sSL -o "$T/android-all.jar" "https://repo1.maven.org/maven2/org/robolectric/android-all/$ANDROID_ALL/android-all-$ANDROID_ALL.jar"
  fi
  echo "==> the Android app's Kotlin against android-all" >&2
  rm -rf "$T/out-android"
  "$KOTLINC" -nowarn "$P/player/src" "$P/scenes/src" "$P/android/src" "$P/app/src/main/kotlin" "$T/rangerthree.kt" -cp "$T/android-all.jar" -d "$T/out-android" 2>&1 | grep -v '^warning' || true
  [[ -f "$T/out-android/rangerthree/demo/MainActivity.class" ]] || { echo "gpu-android: the app did not compile" >&2; exit 1; }
  echo "Android sources OK"
fi
