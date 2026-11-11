# Linux 4.9 bring-up for the Amazing A32

This branch keeps the working Rust userspace and changes only the kernel path.
The first goal is not a fully working phone. It is a repeatable RAM boot of a
Linux 4.9 kernel far enough to run the existing `a32-init` initramfs.

## Source candidate

The initial source candidate is:

- repository: `rokibhasansagar/alps-mp-p0.mp2_kernels`
- branch family: `android-4.9`
- pinned commit: `f42baea1a221dea8b56c44d24d931a537a8aea85`
- upstream description: ALPS kernel sources for MT6737 / MT6753 / MT6739 / MT6580

This is a family source, not an Amazing A32 board source. Do not assume its
defconfig, device tree, display panel, touch, PMIC, camera, Wi-Fi or storage
settings match this phone.

Fetch the pinned tree into the ignored `phone-tools` directory:

```sh
scripts/prepare-kernel-4.9.sh
```

The kernel source must be on a case-sensitive filesystem: the vendor tree
contains both `net/netfilter/xt_HL.c` and `net/netfilter/xt_hl.c`, plus other
case-only filename pairs. Preparation checks this before checkout.
On WSL, use a source directory under `/home`, then link the default path to it:

```sh
bash scripts/prepare-kernel-4.9.sh "$HOME/a32-kernel-4.9"
ln -s "$HOME/a32-kernel-4.9" phone-tools/kernel-4.9
```

If the default source directory already exists on a Windows drive, preserve it
under a different name before creating the link. A fresh checkout restores both
members of each filename pair; copying the damaged working tree does not.
The build output must also be case-sensitive. On WSL, set
`A32_KERNEL49_OUT="$HOME/a32-kernel-4.9-build"` when building, or symlink
`target/kernel-4.9/build` to a directory under `/home`. The build script checks
both directories before compiling.

## Boot-image path

The successful full-build baseline is tagged `kernel-4.9-build-success`.
Its artifact sizes, SHA-256 hashes, actual config hash, pinned source commit,
patch hashes and Linux banner are saved in `kernel-4.9-build-success.json`.
Run `python3 scripts/record-kernel-4.9-build.py` in WSL after subsequent builds
to write `target/kernel-4.9/build-manifest.json`. The recorder checks the gzip
payload against `Image`, the appended DTB, and the exported kernel copy.
The baseline release is `4.9.117-a32-k49+`; the suffix includes the local
compatibility patches. The compressed `zImage` does not expose its banner to
plain `strings`; the recorder reads the decompressed payload instead.
This establishes a build baseline; device boot has not been validated.

The stock boot header remains the template because the working phone has already
proved its load addresses, page size and command line. The host boot-image tool
now has a separate command that replaces both the kernel and the ramdisk while
preserving those header fields:

```text
cargo bootimg repack-kernel TEMPLATE KERNEL RAMDISK_GZ OUT
```

Build a test image after a 4.9 `zImage-dtb` (or equivalent single kernel blob)
exists:

```sh
scripts/build-kernel-4.9-image.sh PATH/TO/zImage-dtb
```

The output is:

```text
target/a32-kernel-4.9-boot.img
```

For bring-up, use RAM boot only:

```sh
fastboot boot target/a32-kernel-4.9-boot.img
```

Do not flash the 4.9 image until RAM boot is repeatable and the recovery path is
confirmed.

## Bring-up order

1. **Source inventory**
   - locate MT6737/MT6737M machine support, defconfigs and DTS/DTSI files;
   - identify the kernel build architecture (the stock system is ARMv7);
   - record the 4.9 image type expected by the vendor boot path.
2. **Minimal kernel build**
   - start from the closest MT6737-family defconfig;
   - enable initramfs, proc, sysfs, tmpfs and the storage/console path needed by
     `a32-init`;
   - build without trying to make display, camera or radio work yet.
3. **First RAM boot**
   - prove LK accepts the boot image;
   - prove the kernel reaches initramfs;
   - capture the earliest available log;
   - if the existing USB gadget path survives, run `a32> info` and
     `a32> compat`.
4. **Board matching**
   - port storage/eMMC first so `/data` can be mounted;
   - then USB gadget/serial, framebuffer, touch, Wi-Fi, audio and camera;
   - keep each driver change separately testable.
5. **Userspace feature config**
   - once the board boots, enable the kernel features that motivated the
     migration: additional namespaces, cgroups, seccomp, overlayfs and tun as
     supported by this 4.9 tree.

## What success means for the first image

A first image is successful even if display, touch, Wi-Fi, Bluetooth, audio and
camera are all broken. The minimum useful milestone is:

```text
LK -> Linux 4.9 -> initramfs -> a32-init
```

The existing 3.18 image remains the recovery/reference system. Compare
`a32> compat` output between the two kernels instead of inferring capabilities
from the version number.

## Known limitation

Linux 4.9 is an intermediate compatibility target, not a modern end state. It is
useful because an MT6737-family vendor tree exists, but syscalls introduced after
4.9 will still be absent. The purpose of this branch is to gain control of the
kernel build and board configuration before attempting a later kernel.
