# Amazing A32: Linux 4.9 artifacts and first-boot preparation

Inspected on 2026-11-11, branch `kernel-4.9`. No device commands were run.
**Build integrity is verified; the current reference-board kernel is not ready
for a phone boot test.** No new boot image was generated from signed stock input.

## Reproduce the offline checks

Run in Linux/WSL with Bash, Python 3, make and the prepared kernel tree:

```sh
bash scripts/verify-kernel-artifacts.sh
python3 scripts/boot-test-image.py inspect phone-tools/backup/boot-mmcblk0p7.img
python3 scripts/test-boot-test-image.py
```

`A32_KERNEL49_DIR` and `A32_KERNEL49_OUT` override the source and output paths.
In this checkout they resolve to `/home/morefunfun/.cache/a32-kernel-casefix/source`
and `/home/morefunfun/.cache/a32-kernel-casefix/build`. Keep these on a
case-sensitive filesystem. Build products and private partition images stay ignored.

For the raw file/size/hash checks used in this audit:

```sh
file target/kernel-4.9/build/arch/arm/boot/{Image,zImage,zImage-dtb}
ls -lh target/kernel-4.9/build/arch/arm/boot/{Image,zImage,zImage-dtb}
sha256sum target/kernel-4.9/build/arch/arm/boot/{Image,zImage,zImage-dtb}
make -s -C phone-tools/kernel-4.9 kernelversion
```

## Architecture, version and artifacts

The build script passes `ARCH=arm`, uses `arm-linux-androideabi-` GCC 4.9,
and requests `zImage-dtb`. Actual `.config` selects `CONFIG_ARM=y`,
`CONFIG_CPU_V7=y`, `CONFIG_CPU_32v7=y`, `CONFIG_MACH_MT6735M=y`.
Both compressed images are identified by `file` as Linux ARM little-endian
zImage executables. This is an **ARM32 / ARMv7 kernel**, even though the DTS
describes Cortex-A53 CPUs. ARM64 CPU capability does not determine kernel ABI.

`make kernelversion` returns **4.9.117**. The gzip payload of zImage was
decompressed and compared byte for byte with `Image`; it contains:

```text
Linux version 4.9.117-a32-k49+ (morefunfun@r5) (gcc version 4.9.x 20150123 (prerelease) (GCC) ) #1 SMP PREEMPT Wed Nov 11 21:31:02 CST 2026
```

Paths below are relative to `target/kernel-4.9/build/`:

| Artifact | Bytes | SHA256 |
| --- | ---: | --- |
| `arch/arm/boot/Image` | 17633280 | `1854a8887b4d0fb2bec087e9fe92ff46239d34d838a349d90f3a2236d6c5cf52` |
| `arch/arm/boot/zImage` | 6978536 | `4247b4a054dc36f84bfbe783142479f17861909558109bb312dbe9abea8dc54e` |
| `arch/arm/boot/zImage-dtb` | 7025796 | `903407f54fac63359a73dc20df082e10d44bd0176e7bcb2dd0e72ceaa2a069fc` |
| `arch/arm/boot/dts/mt6735m.dtb` | 47260 | `162c70791c87a7bd96dc1d2980f7747b80cd2e43d93f328981d332cdffd96ef1` |
| `arch/arm/boot/dts/k37mv1_bsp_k49.dtb` | 36356 | `d8ee9405350958187f38a7b630168272f62fb3a1a53cf18f9616a50d4775fbf1` |
| `arch/arm/boot/dts/k37mv1_bsp_k49.dtb.merge` (overlay validation) | 83616 | `1fbfae96b2c3f728c0ac6a501aa4df5ebfb779c106a0984f3a471aaa9d24bfdf` |

The exported `target/kernel-4.9/zImage-dtb` matches the build output exactly
(about 6.70 MiB). `file Image` reports `data`; its integrity and Linux banner
are established by the decompression comparison above.
`arch/arm64/boot/Image`, `Image.gz` and `Image.gz-dtb` are all absent.

## Which DTB actually belongs to A32?

There is **no verified A32-specific 4.9 DTB** in this build. The strongest A32
reference is the DTB inside the existing device boot-partition backup:

- Input: `phone-tools/backup/boot-mmcblk0p7.img`.
- Kernel section starts at file offset 2048; its zImage ends at kernel-relative
  offset 6695352. A single 60245-byte FDT follows and ends exactly at the end
  of the kernel section.
- FDT SHA256: `2bfe064a24aa130fd126b05d54ef3834b5fce674a22211254a115147fc34c3ba`.
- Extracted locally as `target/kernel-4.9/stock-a32.dtb` for inspection.
  Root model is `MT6735M`, compatible `mediatek,MT6735`; chosen bootargs select
  `ttyMT0`. The generic model alone cannot identify the board; provenance from
  the device partition makes this the best available A32 DT reference.
- Vendor DT has legacy validation warnings when decompiled by the 4.9 dtc.
  Forced decompilation was used only for inspection, not to regenerate a DTB.

The 4.9 output instead has this evidence chain:

1. `.config`: `BUILD_ARM_APPENDED_DTB_IMAGE_NAMES="mt6735m"`,
   `BUILD_ARM_DTB_OVERLAY_IMAGE_NAMES="k37mv1_bsp_k49"`,
   `ARCH_MTK_PROJECT="k37mv1_bsp_k49"`, `MTK_DTBO_FEATURE=y`.
2. `arch/arm/boot/Makefile` makes `zImage-dtb` by concatenating `zImage` and
   the selected base DTB. Byte comparison proves the result is exactly
   `zImage + mt6735m.dtb`, with no k37 overlay appended.
3. `mt6735m.dts` describes the family base. `k37mv1_bsp_k49.dts` uses `/plugin/`
   under this config and contains reference-board overrides; its include of
   `mt6735m.dts` applies only when DTBO is disabled. It is not a standalone A32 DT.
4. Build log shows both DTC targets, overlay validation using `ufdt_apply_overlay`,
   and warnings about reference GPU/framebuffer register/address cells. A
   `k37mv1_bsp_k49.dtb.merge` validation output also exists. `dtboimg.cfg`
   selects k37 with `id=0`; this does not prove the A32 bootloader loads it.
5. [Board gap audit](kernel-4.9-board-gap.md) records unverified k37 panel,
   geometry, sensors and cameras. No Android `BoardConfig.mk` or A32 Android
   device tree was found in the tracked project configuration.

The likely stock strategy is **DTB appended to zImage inside the boot.img kernel
section**, not a separate boot-header DTB section. Stock header word at offset
40 is zero; there is no second stage. The 4.9 config has
`CONFIG_ARM_APPENDED_DTB` disabled, so image concatenation alone does not prove
the kernel consumes that tail itself. LK must select/pass the FDT (potentially
patching it), or a reviewed board/boot setup must provide it. Exactly how this
device's LK handles the new base/overlay scheme is still unverified. Do not
reuse a 3.18 DTB with 4.9 without a binding/driver compatibility review.

## Stock images and existing packaging tools

Local files already exist; no fresh device read was needed:

| Input | Bytes | SHA256 |
| --- | ---: | --- |
| `phone-tools/backup/boot-mmcblk0p7.img` | 16777216 | `79ef60fc9b0d8330175de1fc8ac60cc839542c24cc4b2993cd3b9772633372d1` |
| `phone-tools/backup/recovery-stock.img` | 16777216 | `fe20fa9eb91697dc859dcd3e3fe9eff77da024589d362057da12ef09919a69cb` |
| OTA `boot.img` inside `phone-tools/firmware/full_esky6737m_35_n-ota-110520.zip` | 8825088 | `fd3b6ae38bc9d9d6978a23f1e9a0dd0b2744924d5156ed2695bb1a89b9daf698` |

The OTA differs from the device backup; it must not silently replace the
device-specific template. Its scatter lists boot/recovery as consecutive 16 MiB
regions and no separate dtbo partition. This is evidence for that firmware layout,
not a substitute for checking the current phone's partition map.

Stock backup header:

| Field | Value |
| --- | --- |
| Magic / header | `ANDROID!`, v0-compatible (offset-40 word = 0) |
| Page size | 2048 |
| Kernel / ramdisk / second bytes | 6755597 / 2060911 / 0 |
| Kernel / ramdisk addresses | `0x40008000` / `0x44000000` |
| Second / tags addresses | `0x40f00000` / `0x4e000000` |
| Name | `091318` |
| Cmdline | `bootopt=64S3,32N2,32N2` |
| OS version raw | `0x0e000128` |
| Header ID | Matches the SHA1 over sections and their sizes |

Boot headers store absolute load addresses, not a unique base/offset tuple.
The repacker retains those bytes rather than inventing a base. For example,
choosing base `0x40000000` would express kernel offset `0x8000`, ramdisk offset
`0x04000000`, second offset `0x00f00000` and tags offset `0x0e000000`, but that
choice is not an independently encoded original parameter.

The backup payload ends at offset **8820736**. Its tail begins with an ASN.1
DER sequence (1320 bytes, version integer 1, Android certificate), followed by
padding and another nonzero 256-byte block at tail offset 2048. The latter's
vendor meaning is unverified. Recovery and OTA boot also have nonzero trailers.
Changing the kernel invalidates content signatures. Keeping the old tail or
silently stripping this vendor data would be incorrect; the new script stops.

Existing `tools/bootimg` is a Rust v0 utility; its old repack path replaces
ramdisk and drops trailers. It is not used by the new workflow. No Rust code
was changed. `build-kernel-4.9-image.sh` now delegates to the strict kernel-only
script with explicit stock-image input, so it no longer builds a Rust ramdisk.
Existing `scripts/ram-boot.ps1` can reboot the phone and boot an image; it was
not run. No standalone Android Image Kitchen, AnyKernel, mkbootimg/unpackbootimg
or Android BoardConfig-based packaging setup was found in the project inventory.

## Strict offline packaging

```sh
bash scripts/prepare-boot-test.sh STOCK_BOOT.img \
  target/kernel-4.9/zImage-dtb out/boot-kernel-4.9-test.img
```

The parser/repacker supports only unwrapped ARM32 Android v0 images with
verified section ID, recognized zImage/appended-DTB layout, zero padding and
no nonzero trailer. It rejects v1-v4, legacy separate-DT header layouts,
vendor wrappers, signatures and unknown data. This is a deliberate supported
format boundary. It has no implicit parameter defaults for boot headers.

For supported input it unpacks sections in memory, preserves the complete
header page, ramdisk and second stage, replaces the kernel section, updates
only kernel size and section ID, and re-parses/compares before writing. An
appended DTB is part of the supplied kernel section; the script checks that
the appended-DTB strategy is unchanged, but cannot certify board compatibility.
Separate header DTB formats are rejected rather than guessed. It preserves
the original zero-filled image extent and refuses an oversized payload or
an existing output/report. The stock input is never modified.

On success, stdout and `OUTPUT.img.json` contain original/new/kernel SHA256,
kernel and final image sizes, both header reports and section hashes. The
default output is `out/boot-kernel-4.9-test.img`. There is no device access.

**Current real-input result:** stock inspection succeeds and the prepare command
returns exit 2 on the signing/vendor trailer, with no output image. This is not
a successful boot-image milestone. Resolving the actual signing/bootloader
requirements must precede extending the parser; there is no force/strip switch.

The artifact verifier checks architecture/config, make version, gzip payload,
banner, selected DTB concatenation, kernel size sanity (1-32 MiB, not a partition
capacity claim), exported-copy consistency, and inventories all requested
ARM/ARM64 paths and DTBs with hashes. Six offline tests cover preservation,
second-stage handling, signatures, unsupported headers, corruption/truncation,
oversize and architecture/DTB-strategy rejection. Synthetic fixtures exist only
inside temporary test directories; none is presented as a device boot image.

## What remains before the first phone test

1. Use the stock DT as reference to create/review an A32 minimal 4.9 DT/config,
   disable or replace unverified k37 peripherals, and rebuild. Resolve LK FDT
   selection/overlay handling. This change intentionally does not alter kernel
   sources to guess those board details.
2. Identify both stock trailer structures and establish an explicit unsigned
   RAM-boot or signing method accepted by this phone. Confirm bootloader lock
   state, RAM-boot support and recovery access through separately authorized
   device inspection. A signed stock template alone cannot sign a changed image.
3. Confirm current boot/recovery images and hashes match these backups; if they
   are stale, obtain read-only full **boot** and **recovery** partition images,
   plus partition map. Existing `lk.img` and `seccfg.img` backups can help analyze
   boot/signature behavior; refresh those read-only if firmware changed. Collect
   a DTB/DTBO partition image only if the actual partition map shows one. There
   is no evidence to demand vendor_boot/init_boot for this legacy layout.
4. Capture the running stock device tree (`/sys/firmware/fdt` or
   `/proc/device-tree`), `/proc/cmdline`, config if exposed, stock boot log and
   earliest UART/USB logging path. This distinguishes bootloader-patched DT
   from the stored appended DTB. Preserve stock ramdisk and verify the 4.9
   Android/init/modules/verity expectations; packaging alone cannot establish
   compatibility with stock 3.18 userspace.
5. Validate the resulting image offline, confirm it fits the actual boot
   capacity, and arrange a known recovery path before issuing a device command.

Once all prerequisites are resolved, prefer this non-permanent test **if the
device supports it** (shown only; never executed by these scripts):

```sh
fastboot boot out/boot-kernel-4.9-test.img
```

Do not run it against the current k37 build. Do not use `ram-boot.ps1` as an
implicit prerequisite step. No flash, partition writes or automatic reboots
are part of this workflow.

Header/layout reference: [AOSP bootimg.h](https://android.googlesource.com/platform/system/tools/mkbootimg/+/refs/heads/main/include/bootimg/bootimg.h)
and [AOSP mkbootimg](https://android.googlesource.com/platform/system/tools/mkbootimg/+/refs/heads/main/mkbootimg.py).
These define the v0 section layout and section-size SHA1 convention; device
trailer and board conclusions above come from local inspection.
