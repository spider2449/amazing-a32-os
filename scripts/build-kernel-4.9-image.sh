#!/usr/bin/env bash
# Compatibility entry point: requires a stock image and preserves its userspace.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [[ $# -lt 2 || $# -gt 3 ]]; then
  echo "usage: $0 STOCK_BOOT.img KERNEL_IMAGE [OUTPUT.img]" >&2
  echo "See docs/kernel-4.9-boot-test.md; signed/vendor images fail closed." >&2
  exit 2
fi
exec bash "$ROOT/scripts/prepare-boot-test.sh" "$@"
