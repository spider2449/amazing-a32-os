#!/usr/bin/env bash
# Builds a RAM-boot test image with an externally built Linux 4.9 kernel and the
# existing Rust initramfs. It never flashes or talks to the phone.
set -euo pipefail

cd "$(dirname "$0")/.."

KERNEL_IMAGE="${1:-${A32_KERNEL49_IMAGE:-}}"
if [[ -z "$KERNEL_IMAGE" ]]; then
  cat >&2 <<'EOF'
usage: scripts/build-kernel-4.9-image.sh PATH_TO_ZIMAGE_DTB

or set A32_KERNEL49_IMAGE. The kernel must already be built; this script only
combines it with the existing Rust initramfs and the stock boot header.
EOF
  exit 2
fi

if [[ ! -f "$KERNEL_IMAGE" ]]; then
  echo "kernel image not found: $KERNEL_IMAGE" >&2
  exit 2
fi

OTA="phone-tools/firmware/full_esky6737m_35_n-ota-110520.zip"
OUT="target/kernel-4.9"
BOOT_OUT="target/a32-kernel-4.9-boot.img"

mkdir -p "$OUT"
unzip -p "$OTA" boot.img > "$OUT/stock-boot.img"

cargo build --release
cargo bootimg mkinitramfs   target/armv7-unknown-linux-musleabihf/release/a32-init "$OUT/initramfs.cpio"
gzip -9nc "$OUT/initramfs.cpio" > "$OUT/initramfs.cpio.gz"

cargo bootimg repack-kernel   "$OUT/stock-boot.img"   "$KERNEL_IMAGE"   "$OUT/initramfs.cpio.gz"   "$BOOT_OUT"

echo
cargo bootimg info "$BOOT_OUT"
echo
echo "built $BOOT_OUT"
echo "test with RAM boot only: fastboot boot $BOOT_OUT"
