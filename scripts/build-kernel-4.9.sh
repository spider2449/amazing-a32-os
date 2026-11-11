#!/usr/bin/env bash
# Builds the pinned MT6737-family Linux 4.9 candidate far enough to produce
# arch/arm/boot/zImage-dtb. It never creates a boot image and never talks to
# the phone; see docs/kernel-4.9-boot-test.md for packaging prerequisites.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

KERNEL_DIR="${A32_KERNEL49_DIR:-$ROOT/phone-tools/kernel-4.9}"
OUT="${A32_KERNEL49_OUT:-$ROOT/target/kernel-4.9/build}"
CONFIG_FILE="${A32_KERNEL49_CONFIG:-$ROOT/kernel-4.9/configs/a32_bringup_defconfig}"
CONFIG_FRAGMENT="${A32_KERNEL49_FRAGMENT:-}"
DEFAULT_CROSS="$ROOT/toolchains/arm-linux-androideabi-4.9/bin/arm-linux-androideabi-"
if [[ -n "${A32_CROSS_COMPILE:-}" ]]; then
  CROSS="$A32_CROSS_COMPILE"
elif [[ -x "${DEFAULT_CROSS}gcc" ]]; then
  CROSS="$DEFAULT_CROSS"
else
  echo "Android GCC 4.9 toolchain is not prepared." >&2
  echo "run: scripts/prepare-kernel-4.9-toolchain.sh" >&2
  echo "or set A32_CROSS_COMPILE explicitly." >&2
  exit 2
fi
JOBS="${A32_KERNEL49_JOBS:-}"
VERBOSE="${A32_KERNEL49_VERBOSE:-0}"
PINNED="f42baea1a221dea8b56c44d24d931a537a8aea85"
DRVGEN_WRAPPER="$ROOT/scripts/mtk-drvgen-wrapper.sh"
PREFLIGHT="$ROOT/scripts/kernel-4.9-preflight.sh"
LOG="$ROOT/target/kernel-4.9/build.log"

bash "$ROOT/scripts/kernel-4.9-check-filesystem.sh" "$KERNEL_DIR"
bash "$ROOT/scripts/kernel-4.9-check-filesystem.sh" "$OUT"

if [[ -f "$PREFLIGHT" ]]; then
  echo "== host preflight =="
  bash "$PREFLIGHT"
  echo
fi

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
if [[ "$VERBOSE" == "1" ]]; then
  MAKE_ARGS+=(V=1)
fi

echo "kernel:       $KERNEL_DIR" | tee -a "$LOG"
echo "commit:       $head" | tee -a "$LOG"
echo "config:       $CONFIG_FILE" | tee -a "$LOG"
if [[ -n "$CONFIG_FRAGMENT" ]]; then
  echo "fragment:     $CONFIG_FRAGMENT" | tee -a "$LOG"
else
  echo "fragment:     (none)" | tee -a "$LOG"
fi
echo "cross gcc:    $(command -v "${CROSS}gcc")" | tee -a "$LOG"
"${CROSS}gcc" --version | head -n 1 | tee -a "$LOG"
if command -v file >/dev/null 2>&1; then
  file "$KERNEL_DIR/tools/dct/old_dct/DrvGen" | tee -a "$LOG"
fi

if [[ ! -f "$CONFIG_FILE" ]]; then
  echo "kernel config not found: $CONFIG_FILE" >&2
  exit 2
fi
if [[ -n "$CONFIG_FRAGMENT" && ! -f "$CONFIG_FRAGMENT" ]]; then
  echo "kernel config fragment not found: $CONFIG_FRAGMENT" >&2
  exit 2
fi

echo | tee -a "$LOG"
echo "== A32 bring-up config ==" | tee -a "$LOG"
if [[ -n "$CONFIG_FRAGMENT" ]]; then
  KCONFIG_CONFIG="$OUT/.config" \
    "$KERNEL_DIR/scripts/kconfig/merge_config.sh" -m -O "$OUT" \
    "$CONFIG_FILE" "$CONFIG_FRAGMENT" 2>&1 | tee -a "$LOG"
else
  cp "$CONFIG_FILE" "$OUT/.config"
fi
make "${MAKE_ARGS[@]}" olddefconfig 2>&1 | tee -a "$LOG"

echo | tee -a "$LOG"
echo "== selected ARM / MediaTek settings ==" | tee -a "$LOG"
grep -E 'CONFIG_(CPU_V7|CPU_32v7|ARCH_MEDIATEK|MACH_MT6735M|MTK_DTBO_FEATURE|BUILD_ARM_APPENDED_DTB_IMAGE|BUILD_ARM_APPENDED_DTB_IMAGE_NAMES|BUILD_ARM_DTB_OVERLAY_IMAGE|BUILD_ARM_DTB_OVERLAY_IMAGE_NAMES|ARCH_MTK_PROJECT|MTK_SERIAL|MMC|MMC_MTK_PRO|MTK_EMMC_SUPPORT|EXT4_FS|USB_GADGET|USB_CONFIGFS_SERIAL|USB_CONFIGFS_ACM)=' "$OUT/.config" | tee -a "$LOG" || true

if [[ -n "$CONFIG_FRAGMENT" ]]; then
  echo | tee -a "$LOG"
  echo "== minimal profile should be disabled ==" | tee -a "$LOG"
  grep -E 'CONFIG_(MTK_FB|MTK_LCM|TOUCHSCREEN_MTK|MTK_IMGSENSOR|SOUND|SND|HID|MTK_COMBO|MTK_ECCCI_DRIVER|MTK_GPU_SUPPORT)=' "$OUT/.config" | tee -a "$LOG" || true
fi

echo | tee -a "$LOG"
echo "== compiler ARMv7 probe ==" | tee -a "$LOG"
if printf 'int x;\n' | "${CROSS}gcc" -march=armv7-a -x c -c -o /dev/null - 2>>"$LOG"; then
  echo "armv7-a accepted by ${CROSS}gcc" | tee -a "$LOG"
else
  echo "armv7-a rejected by ${CROSS}gcc" | tee -a "$LOG"
fi

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
echo "next:  bash scripts/verify-kernel-artifacts.sh"
echo "boot packaging prerequisites: docs/kernel-4.9-boot-test.md"
