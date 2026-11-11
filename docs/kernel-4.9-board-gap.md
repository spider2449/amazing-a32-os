# Amazing A32 Linux 4.9 board gap

This file separates **compile proof** from **phone bootability**.

The pinned MediaTek Linux 4.9 tree is useful because it supports the MT6735M /
MT6737 family and can produce the vendor-style ARM32 `zImage-dtb`.  Its k37
board, however, is not the Amazing A32.

## Hardware confirmed on the working A32

| Function | Amazing A32 |
|---|---|
| SoC | MediaTek MT6737M, ARM32 kernel |
| Display geometry | 480x854 |
| Touch input | Linux input name `mtk-tpd`; controller model not yet identified |
| Accelerometer | MXC400X |
| ALS / proximity | AP3426 |
| Rear camera | GC5025, 2592x1944 |
| Front camera | GC2385, 1600x1200 |
| eMMC | MediaTek eMMC path is already proven by the stock 3.18 system |
| Serial / USB | MediaTek serial and USB gadget paths are already used by the Rust init |

## What the current 4.9 reference config contains

The compile-only baseline still carries k37 reference-board choices including:

- `hx8392a_dsi_cmd_qhd` LCM;
- 540x960 LCM geometry;
- GT1151 touch;
- MC3410 accelerometer;
- CM36652 ALS/proximity;
- IMX135 / GC2355 / IMX219 camera list.

These values are **not Amazing A32 hardware facts**.

The pinned 4.9 source was also checked directly and does not contain the known A32
MXC400X, AP3426, GC5025 or GC2385 drivers.  Those drivers therefore need to be
ported from a compatible source or recreated against the 4.9 MediaTek frameworks.

## Safety rule

`kernel-4.9/configs/a32_bringup_defconfig` is currently a **compile-only
reference configuration**.  A successful CI build proves that the vendor kernel
and host tools can build.  It does not make the produced kernel safe to boot on
the A32.

Do not RAM-boot a kernel that still selects the unverified k37 panel and board
peripherals.  First create an A32 boot configuration that either disables those
peripherals or replaces them with known A32 data.

## Porting order after compile succeeds

1. Keep only SoC, interrupt, clock, timer, eMMC, serial and minimum USB support.
2. Make a first A32 DT/overlay with storage and console only.
3. RAM boot and prove `LK -> Linux 4.9 -> /init -> a32-init`.
4. Add the framebuffer/LCM only after the actual A32 panel is identified.
5. Identify and add the touch controller.
6. Port MXC400X and AP3426.
7. Port Wi-Fi/Bluetooth board data and calibration handling.
8. Port GC5025 and GC2385 last.

The camera stack is deliberately late: both sensors are known, but the sensor
drivers are absent from the pinned 4.9 tree and camera bring-up also depends on
board power sequencing, MIPI lane configuration and ISP integration.

## Useful checks

After fetching the kernel tree:

```sh
scripts/kernel-4.9-board-audit.sh
```

For the compile-only baseline:

```sh
scripts/kernel-4.9-preflight.sh
scripts/build-kernel-4.9.sh
```

Only after an A32-specific boot configuration exists should
`scripts/build-kernel-4.9-image.sh` be used for a RAM-boot test.
