#!/usr/bin/env bash
# Audit the pinned Linux 4.9 source for hardware known to exist in the Amazing A32.
# This is intentionally separate from the compile preflight: a source tree can
# compile successfully while still lacking the phone-specific peripheral drivers.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_DIR="${A32_KERNEL49_DIR:-$ROOT/phone-tools/kernel-4.9}"

if [[ ! -d "$KERNEL_DIR" ]]; then
  echo "kernel source is missing: $KERNEL_DIR" >&2
  echo "run: scripts/prepare-kernel-4.9.sh" >&2
  exit 2
fi

found=0
missing=0
unknown=0

check_driver() {
  local label="$1"
  shift
  local patterns=("$@")
  local hit=""

  for pattern in "${patterns[@]}"; do
    hit="$(grep -RIl --exclude-dir=.git -- "$pattern" "$KERNEL_DIR" 2>/dev/null | head -n 1 || true)"
    if [[ -n "$hit" ]]; then
      printf '  [found]   %-22s %s\n' "$label" "${hit#$KERNEL_DIR/}"
      found=$((found + 1))
      return 0
    fi
  done

  printf '  [missing] %-22s searched: %s\n' "$label" "${patterns[*]}"
  missing=$((missing + 1))
}

echo "Amazing A32 -> pinned Linux 4.9 hardware audit"
echo

echo "Known from the working stock 3.18 system:"
echo "  SoC:          MT6737M / ARM32"
echo "  display:      480x854"
echo "  accelerometer: MXC400X"
echo "  ALS/proximity: AP3426"
echo "  rear camera:   GC5025"
echo "  front camera:  GC2385"
echo "  touch input:   mtk-tpd (controller model still unknown)"
echo

echo "== driver presence =="
check_driver "MXC400X accel" "MXC400X" "mxc400x"
check_driver "AP3426 ALS/PS" "AP3426" "ap3426"
check_driver "GC5025 rear camera" "GC5025" "gc5025"
check_driver "GC2385 front camera" "GC2385" "gc2385"

echo
echo "== reference-config mismatches =="
CONFIG="$ROOT/kernel-4.9/configs/a32_bringup_defconfig"
if [[ -f "$CONFIG" ]]; then
  grep -E '^CONFIG_(CUSTOM_KERNEL_LCM|LCM_HEIGHT|LCM_WIDTH|TOUCHSCREEN_MTK_|CUSTOM_KERNEL_ACCELEROMETER|MTK_MC3410|CUSTOM_KERNEL_ALSPS|MTK_CM36652|CUSTOM_KERNEL_IMGSENSOR)=' "$CONFIG" || true
else
  echo "bring-up config missing: $CONFIG"
fi

echo
echo "summary: $found known A32 peripheral driver(s) found, $missing missing"
echo
cat <<'EOF'
Interpretation:
  - This audit is informational and does not block the compile-only CI.
  - The reference k37 config is for proving that the vendor 4.9 tree builds.
  - Do NOT treat its panel/touch/sensor/camera choices as Amazing A32 board data.
  - Before RAM boot, replace or disable unverified peripheral drivers and port the
    A32 board data from the stock 3.18 environment.
EOF
