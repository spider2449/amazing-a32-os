#!/usr/bin/env bash
# Fetches the first MT6737-family Linux 4.9 source candidate into phone-tools.
# The source is kept outside git because it is large and is not yet proven to
# match the Amazing A32 board.
set -euo pipefail

cd "$(dirname "$0")/.."

DEST="${1:-phone-tools/kernel-4.9}"
REPO="https://github.com/rokibhasansagar/alps-mp-p0.mp2_kernels.git"
REF="android-4.9"
COMMIT="f42baea1a221dea8b56c44d24d931a537a8aea85"

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
  echo "unexpected kernel commit: $actual" >&2
  exit 2
fi

echo "kernel source: $DEST"
echo "source repo:   $REPO"
echo "source ref:    $REF"
echo "commit:        $actual"
