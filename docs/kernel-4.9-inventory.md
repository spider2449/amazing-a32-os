# MT6737-family Linux 4.9 source inventory

Source pinned by `scripts/prepare-kernel-4.9.sh`:

- repository: `rokibhasansagar/alps-mp-p0.mp2_kernels`
- branch: `android-4.9`
- commit: `f42baea1a221dea8b56c44d24d931a537a8aea85`
- kernel release in the tree: **4.9.117**

## Closest 32-bit MT6737-family target

The source tree contains:

```text
arch/arm/configs/k37mv1_bsp_k49_defconfig
arch/arm/configs/k37mv1_bsp_k49_debug_defconfig
arch/arm/boot/dts/mt6735m.dts
arch/arm/boot/dts/k37mv1_bsp_k49.dts
drivers/misc/mediatek/dws/mt6735/k37mv1_bsp_k49.dws
```

The normal defconfig selects:

```text
CONFIG_MACH_MT6735M=y
CONFIG_BUILD_ARM_APPENDED_DTB_IMAGE=y
CONFIG_BUILD_ARM_APPENDED_DTB_IMAGE_NAMES="mt6735m"
CONFIG_BUILD_ARM_DTB_OVERLAY_IMAGE_NAMES="k37mv1_bsp_k49"
CONFIG_ARCH_MTK_PROJECT="k37mv1_bsp_k49"
CONFIG_MTK_FB=y
CONFIG_MMC=y
CONFIG_MMC_MTK_PRO=y
CONFIG_MTK_EMMC_SUPPORT=y
CONFIG_EXT4_FS=y
CONFIG_TMPFS=y
```

`CONFIG_MTK_DTBO_FEATURE` is defined by the MediaTek Kconfig with `default y`.
That means the intended layout is:

```text
mt6735m.dtb                 base SoC DT
        +
k37mv1_bsp_k49.dtb         project overlay
        |
        +-- generated k37mv1_bsp_k49/cust.dtsi
```

The ARM build then concatenates the base DTB to the compressed kernel and
produces:

```text
arch/arm/boot/zImage-dtb
```

This matches the image shape needed by the new `bootimg repack-kernel` path.

## Why k37 is only a bring-up reference

It is close enough to be valuable at the SoC level:

- ARM32 kernel;
- Cortex-A53 CPU nodes at 1.1 GHz;
- MT6735M/MT6737-family MediaTek drivers;
- MTK eMMC, framebuffer, USB and radio driver families.

It is **not** the Amazing A32 board description. The reference overlay contains
hardware choices that already disagree with the phone, for example:

- reference touch resolution: 720x1280;
- Amazing A32 framebuffer used by this project: 480x854;
- reference LCM include: `otm9608_qhd_dsi_cmd.dtsi`.

Therefore the first 4.9 test must not be treated as a flashable board port.

## MediaTek DCT / DrvGen issue

The overlay includes:

```text
#include <k37mv1_bsp_k49/cust.dtsi>
```

That file is generated from:

```text
drivers/misc/mediatek/dws/mt6735/k37mv1_bsp_k49.dws
```

The vendor `tools/dct/DrvGen.py` front-end uses Python-2 syntax. The k37 DWS is
an old-format DWS, for which that script delegates to the bundled binary:

```text
tools/dct/old_dct/DrvGen
```

To avoid making Python 2 a host dependency, this repository provides
`scripts/mtk-drvgen-wrapper.sh`. The kernel build passes that wrapper through
the vendor make variable named `python`; the wrapper skips the Python front-end
and calls `old_dct/DrvGen` with the same arguments.

If the bundled binary itself cannot execute on the host (for example because of
an old ELF loader dependency), the build log will classify the failure at the
DrvGen stage and the next task is to reproduce `cust.dtsi` generation without
the binary.

## First compile

After fetching the source:

```sh
scripts/prepare-kernel-4.9.sh
scripts/build-kernel-4.9.sh
```

The build log is kept at:

```text
target/kernel-4.9/build.log
```

On success the copied kernel image is:

```text
target/kernel-4.9/zImage-dtb
```

Then package it, still without flashing:

```sh
scripts/build-kernel-4.9-image.sh target/kernel-4.9/zImage-dtb
```

and use RAM boot only:

```sh
fastboot boot target/a32-kernel-4.9-boot.img
```

## First runtime milestone

The first useful result is not a working screen. It is one of these, in order:

1. LK accepts the image rather than rejecting/restarting immediately.
2. Linux prints or exposes any early boot evidence.
3. The kernel reaches `/init`.
4. Existing USB serial comes up and `a32> info` reports Linux 4.9.
5. `a32> compat` produces a before/after capability report.

Only after that should the A32-specific board data be ported from the stock
3.18 environment into an A32 4.9 overlay.
