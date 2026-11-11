#!/usr/bin/env bash
# Preflight checks for the Amazing A32 Linux 4.9 bring-up.
#
# This does not build or modify the kernel tree. It only verifies the host-side
# pieces that commonly stop old MediaTek vendor kernels before C compilation.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

KERNEL_DIR="${A32_KERNEL49_DIR:-$ROOT/phone-tools/kernel-4.9}"
CONFIG_FILE="${A32_KERNEL49_CONFIG:-$ROOT/kernel-4.9/configs/a32_bringup_defconfig}"
DEFAULT_CROSS="$ROOT/toolchains/arm-linux-androideabi-4.9/bin/arm-linux-androideabi-"
if [[ -n "${A32_CROSS_COMPILE:-}" ]]; then
  CROSS="$A32_CROSS_COMPILE"
elif [[ -x "${DEFAULT_CROSS}gcc" ]]; then
  CROSS="$DEFAULT_CROSS"
else
  CROSS="$DEFAULT_CROSS"
fi
PINNED="f42baea1a221dea8b56c44d24d931a537a8aea85"
DRVGEN="$KERNEL_DIR/tools/dct/old_dct/DrvGen"

errors=0
warnings=0

ok()   { printf '  [ok]   %s\n' "$*"; }
warn() { printf '  [warn] %s\n' "$*"; warnings=$((warnings + 1)); }
fail() { printf '  [fail] %s\n' "$*"; errors=$((errors + 1)); }

echo "A32 Linux 4.9 host preflight"
echo "kernel: $KERNEL_DIR"
echo "config: $CONFIG_FILE"
echo "cross:  $CROSS"
echo

echo "== required host commands =="
for cmd in git make perl bc sed grep awk gzip; do
  if command -v "$cmd" >/dev/null 2>&1; then
    ok "$cmd -> $(command -v "$cmd")"
  else
    fail "$cmd is missing"
  fi
done

for cmd in bison flex openssl file readelf; do
  if command -v "$cmd" >/dev/null 2>&1; then
    ok "$cmd -> $(command -v "$cmd")"
  else
    warn "$cmd is missing; the vendor tree may need it later in the build"
  fi
done

echo
echo "== kernel source =="
if [[ ! -d "$KERNEL_DIR/.git" ]]; then
  fail "kernel source is missing; run scripts/prepare-kernel-4.9.sh"
else
  head="$(git -C "$KERNEL_DIR" rev-parse HEAD 2>/dev/null || true)"
  if [[ "$head" == "$PINNED" ]]; then
    ok "pinned source commit $head"
  else
    fail "source HEAD is ${head:-unknown}; expected $PINNED"
  fi
fi

if [[ -f "$CONFIG_FILE" ]]; then
  ok "bring-up config exists"
else
  fail "bring-up config is missing"
fi

echo
echo "== ARM cross compiler =="
gcc="${CROSS}gcc"
if command -v "$gcc" >/dev/null 2>&1; then
  gcc_path="$(command -v "$gcc")"
  gcc_line="$("$gcc" --version 2>/dev/null | head -n 1 || true)"
  ok "$gcc_path"
  printf '         %s\n' "$gcc_line"

  major="$("$gcc" -dumpversion 2>/dev/null | cut -d. -f1 || true)"
  if [[ "$major" =~ ^[0-9]+$ ]] && (( major >= 10 )); then
    warn "GCC $major is much newer than this vendor 4.9 tree; the CI-proven compiler is Android GCC 4.9 from scripts/prepare-kernel-4.9-toolchain.sh"
  fi
else
  fail "cross compiler not found: $gcc"
  printf '         run scripts/prepare-kernel-4.9-toolchain.sh\n'
  printf '         or set A32_CROSS_COMPILE to another tested compiler prefix\n'
fi

echo
echo "== MediaTek old DrvGen =="
if [[ ! -e "$DRVGEN" ]]; then
  fail "DrvGen is missing: $DRVGEN"
elif [[ ! -x "$DRVGEN" ]]; then
  fail "DrvGen exists but is not executable: $DRVGEN"
else
  ok "DrvGen exists and is executable"

  if command -v file >/dev/null 2>&1; then
    printf '         %s\n' "$(file "$DRVGEN")"
  fi

  if command -v readelf >/dev/null 2>&1; then
    interp="$(readelf -l "$DRVGEN" 2>/dev/null | sed -n 's/.*Requesting program interpreter: \(.*\)]/\1/p' | head -n 1 || true)"
    if [[ -n "$interp" ]]; then
      if [[ -e "$interp" ]]; then
        ok "DrvGen ELF interpreter exists: $interp"
      else
        fail "DrvGen requires missing ELF interpreter: $interp"
      fi
    else
      warn "could not determine the DrvGen ELF interpreter"
    fi
  fi

  if command -v ldd >/dev/null 2>&1; then
    missing="$(ldd "$DRVGEN" 2>&1 | grep 'not found' || true)"
    if [[ -n "$missing" ]]; then
      fail "DrvGen has missing shared libraries"
      while IFS= read -r line; do printf '         %s\n' "$line"; done <<< "$missing"
    else
      ok "DrvGen shared-library check did not report missing libraries"
    fi
  fi
fi

echo
echo "== bring-up config sanity =="
if [[ -f "$CONFIG_FILE" ]]; then
  for symbol in \
    'CONFIG_MACH_MT6735M=y' \
    'CONFIG_BUILD_ARM_APPENDED_DTB_IMAGE=y' \
    'CONFIG_ARCH_MTK_PROJECT="k37mv1_bsp_k49"' \
    'CONFIG_MMC=y' \
    'CONFIG_MTK_EMMC_SUPPORT=y' \
    'CONFIG_EXT4_FS=y' \
    'CONFIG_TMPFS=y'; do
    if grep -Fqx "$symbol" "$CONFIG_FILE"; then
      ok "$symbol"
    else
      fail "expected config entry is missing: $symbol"
    fi
  done
fi

echo
printf 'summary: %d error(s), %d warning(s)\n' "$errors" "$warnings"

if (( errors != 0 )); then
  cat <<'EOF'

Preflight failed. Fix the [fail] entries before treating a kernel compile error
as a board-port problem.
EOF
  exit 1
fi

cat <<'EOF'

Preflight passed. Next:
  scripts/build-kernel-4.9.sh

Warnings are advisory; keep the build log if compilation still fails:
  target/kernel-4.9/build.log
EOF
