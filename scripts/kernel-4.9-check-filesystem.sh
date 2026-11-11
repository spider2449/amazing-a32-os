#!/usr/bin/env bash
# Check source and output directories before case-only filenames can collide.
set -euo pipefail
DEST="${1:?directory required}"
mkdir -p "$DEST"
probe="$(mktemp -d "$DEST/.a32-case-check.XXXXXX")"
trap 'rm -f "$probe/lower"; rmdir "$probe"' EXIT
touch "$probe/lower"
if [[ -e "$probe/LOWER" ]]; then
  echo "kernel source/build requires a case-sensitive filesystem: $DEST" >&2
  echo "On WSL, use a directory under /home and symlink the expected path to it." >&2
  exit 2
fi
