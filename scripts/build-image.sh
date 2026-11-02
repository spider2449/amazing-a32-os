#!/usr/bin/env bash
# Builds target/a32-boot.img from the stock OTA kernel and the Rust init.
# This only creates local files. It never touches the phone.
set -euo pipefail

cd "$(dirname "$0")/.."

OTA="phone-tools/firmware/full_esky6737m_35_n-ota-110520.zip"
OUT="target/image"

mkdir -p "$OUT"
unzip -p "$OTA" boot.img > "$OUT/stock-boot.img"

cargo build --release
cargo bootimg mkinitramfs \
  target/armv7-unknown-linux-musleabihf/release/a32-init "$OUT/initramfs.cpio"
gzip -9nc "$OUT/initramfs.cpio" > "$OUT/initramfs.cpio.gz"
cargo bootimg repack "$OUT/stock-boot.img" "$OUT/initramfs.cpio.gz" target/a32-boot.img

cargo bootimg info target/a32-boot.img
