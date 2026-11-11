#!/usr/bin/env bash
# Offline only. Never invokes device tools or builds/replaces userspace.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -lt 1 || $# -gt 3 ]]; then
  echo "usage: $0 STOCK_BOOT.img [KERNEL_IMAGE [OUTPUT.img]]" >&2
  exit 2
fi
exec python3 "$ROOT/scripts/boot-test-image.py" prepare "$1" \
  "${2:-$ROOT/target/kernel-4.9/zImage-dtb}" \
  "${3:-$ROOT/out/boot-kernel-4.9-test.img}"
