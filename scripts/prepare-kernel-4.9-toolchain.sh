#!/usr/bin/env bash
# Fetch the CI-proven ARM cross compiler for the Amazing A32 Linux 4.9 tree.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="${1:-$ROOT/toolchains/arm-linux-androideabi-4.9}"
REPO="https://github.com/LineageOS/android_prebuilts_gcc_linux-x86_arm_arm-linux-androideabi-4.9.git"
COMMIT="111258a10e017f067b27e6cfcea7619d753f3309"

if [[ -e "$DEST" && ! -d "$DEST/.git" ]]; then
  echo "$DEST exists but is not a git repository" >&2
  exit 2
fi

if [[ ! -d "$DEST/.git" ]]; then
  mkdir -p "$(dirname "$DEST")"
  git init "$DEST"
  git -C "$DEST" remote add origin "$REPO"
fi

git -C "$DEST" fetch --depth=1 origin "$COMMIT"
git -C "$DEST" checkout --detach FETCH_HEAD

actual="$(git -C "$DEST" rev-parse HEAD)"
if [[ "$actual" != "$COMMIT" ]]; then
  echo "unexpected toolchain commit: $actual" >&2
  exit 2
fi

GCC="$DEST/bin/arm-linux-androideabi-gcc"
if [[ ! -x "$GCC" ]]; then
  echo "toolchain gcc is missing: $GCC" >&2
  exit 2
fi

echo "kernel 4.9 toolchain: $DEST"
echo "toolchain commit:    $actual"
"$GCC" --version | head -n 1
