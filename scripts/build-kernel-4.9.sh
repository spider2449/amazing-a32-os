#!/usr/bin/env bash
# Builds the pinned MT6737-family Linux 4.9 candidate far enough to produce
# arch/arm/boot/zImage-dtb. It never creates a boot image and never talks to
# the phone; use build-kernel-4.9-image.sh for the next step.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

KERNEL_DIR="${A32_KERNEL49_DIR:-$ROOT/phone-tools/kernel-4.9}"
OUT="${A32_KERNEL49_OUT:-$ROOT/target/kernel-4.9/build}"
DEFCONFIG="${A32_KERNEL49_DEFCONFIG:-k37mv1_bsp_k49_defconfig}"
CROSS="${A32_CROSS_COMPILE:-arm-linux-gnueabihf-}"
JOBS="${A32_KERNEL49_JOBS:-}"
PINNED="f42baea1a221dea8b56c44d24d931a537a8aea85"
DRVGEN_WRAPPER="$ROOT/scripts/mtk-drvgen-wrapper.sh"
LOG="$ROOT/target/kernel-4.9/build.log"

if [[ ! -d "$KERNEL_DIR/.git" ]]; then
  echo "kernel source is missing: $KERNEL_DIR" >&2
  echo "run: scripts/prepare-kernel-4.9.sh" >&2
  exit 2
fi

head="$(git -C "$KERNEL_DIR" rev-parse HEAD)"
if [[ "$head" != "$PINNED" ]]; then
  echo "kernel source is at $head, expected pinned commit $PINNED" >&2
  exit 2
fi

if ! command -v make >/dev/null 2>&1; then
  echo "make is required" >&2
  exit 2
fi
if ! command -v "${CROSS}gcc" >/dev/null 2>&1; then
  echo "cross compiler not found: ${CROSS}gcc" >&2
  echo "set A32_CROSS_COMPILE to a working ARM Linux GCC prefix" >&2
  exit 2
fi
if [[ ! -x "$DRVGEN_WRAPPER" ]]; then
  echo "DrvGen wrapper is not executable: $DRVGEN_WRAPPER" >&2
  exit 2
fi

if [[ -z "$JOBS" ]]; then
  if command -v nproc >/dev/null 2>&1; then
    JOBS="$(nproc)"
  else
    JOBS=4
  fi
fi

mkdir -p "$OUT" "$(dirname "$LOG")"
: > "$LOG"

MAKE_ARGS=(
  -C "$KERNEL_DIR"
  O="$OUT"
  ARCH=arm
  CROSS_COMPILE="$CROSS"
  python="$DRVGEN_WRAPPER"
)

echo "kernel:       $KERNEL_DIR" | tee -a "$LOG"
echo "commit:       $head" | tee -a "$LOG"
echo "defconfig:    $DEFCONFIG" | tee -a "$LOG"
echo "cross gcc:    $(command -v "${CROSS}gcc")" | tee -a "$LOG"
"${CROSS}gcc" --version | head -n 1 | tee -a "$LOG"
if command -v file >/dev/null 2>&1; then
  file "$KERNEL_DIR/tools/dct/old_dct/DrvGen" | tee -a "$LOG"
fi

echo | tee -a "$LOG"
echo "== defconfig ==" | tee -a "$LOG"
make "${MAKE_ARGS[@]}" "$DEFCONFIG" 2>&1 | tee -a "$LOG"

echo | tee -a "$LOG"
echo "== selected MediaTek DT settings ==" | tee -a "$LOG"
grep -E 'CONFIG_(MACH_MT6735M|MTK_DTBO_FEATURE|BUILD_ARM_APPENDED_DTB_IMAGE|BUILD_ARM_APPENDED_DTB_IMAGE_NAMES|BUILD_ARM_DTB_OVERLAY_IMAGE|BUILD_ARM_DTB_OVERLAY_IMAGE_NAMES|ARCH_MTK_PROJECT)='   "$OUT/.config" | tee -a "$LOG" || true

echo | tee -a "$LOG"
echo "== zImage-dtb ==" | tee -a "$LOG"
set +e
make "${MAKE_ARGS[@]}" -j"$JOBS" zImage-dtb 2>&1 | tee -a "$LOG"
status=${PIPESTATUS[0]}
set -e
if [[ $status -ne 0 ]]; then
  cat >&2 <<EOF

Linux 4.9 build failed (exit $status).
Full log: $LOG

Useful first classifications:
  - DrvGen / cust.dtsi / old_dct  -> MediaTek DCT host-tool problem
  - dtc / overlay / k37*.dtb     -> device-tree generation problem
  - unknown option / compiler     -> GCC compatibility problem
  - C compile/link error          -> vendor kernel source/config problem
EOF
  exit "$status"
fi

IMAGE="$OUT/arch/arm/boot/zImage-dtb"
if [[ ! -s "$IMAGE" ]]; then
  echo "build reported success but zImage-dtb is missing: $IMAGE" >&2
  exit 2
fi

cp "$IMAGE" "$ROOT/target/kernel-4.9/zImage-dtb"
echo
echo "built: target/kernel-4.9/zImage-dtb"
echo "next:  scripts/build-kernel-4.9-image.sh target/kernel-4.9/zImage-dtb"
