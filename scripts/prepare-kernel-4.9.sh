#!/usr/bin/env bash
# Fetches the first MT6737-family Linux 4.9 source candidate into phone-tools.
# The source is kept outside git because it is large and is not yet proven to
# match the Amazing A32 board.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PATCH_DIR="$ROOT/kernel-4.9/patches"
DEST="${1:-phone-tools/kernel-4.9}"
REPO="https://github.com/rokibhasansagar/alps-mp-p0.mp2_kernels.git"
REF="android-4.9"
COMMIT="f42baea1a221dea8b56c44d24d931a537a8aea85"

if [[ -e "$DEST" && ! -d "$DEST/.git" ]]; then
  echo "$DEST exists but is not a git repository" >&2
  exit 2
fi

# Linux has tracked filenames that differ only by case (e.g. xt_HL.c and
# xt_hl.c). Refuse an unsafe checkout before Git silently overwrites one.
bash "$ROOT/scripts/kernel-4.9-check-filesystem.sh" "$DEST"

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

if compgen -G "$PATCH_DIR/*.patch" >/dev/null; then
  echo "applying A32 host/build compatibility patches:"
  for patch in "$PATCH_DIR"/*.patch; do
    # Later patches can change adjacent context (0005 follows 0004).
    # For detection only, allow reduced context while matching the actual edit.
    if git -C "$DEST" apply --reverse --check -C0 "$patch" >/dev/null 2>&1; then
      echo "  already applied: $(basename "$patch")"
    elif git -C "$DEST" apply --check "$patch" >/dev/null 2>&1; then
      git -C "$DEST" apply "$patch"
      echo "  applied:         $(basename "$patch")"
    else
      echo "patch conflicts with kernel tree: $patch" >&2
      exit 2
    fi
  done
fi

echo "kernel source: $DEST"
echo "source repo:   $REPO"
echo "source ref:    $REF"
echo "commit:        $actual"

if [[ "${A32_SKIP_KERNEL49_TOOLCHAIN:-0}" != "1" ]]; then
  echo
  echo "== Android GCC 4.9 toolchain =="
  bash "$ROOT/scripts/prepare-kernel-4.9-toolchain.sh"
fi
