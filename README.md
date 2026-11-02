# amazing-a32-os

Linux-based system for the Innovatech/TWM Amazing A32 phone: the stock Linux
kernel with a Rust userspace.

## Device profile

| Item | Value |
|---|---|
| SoC | MediaTek MT6737M, Cortex-A53 cores running a 32-bit kernel |
| ABI / Rust target | `armeabi-v7a` / `armv7-unknown-linux-musleabihf` |
| RAM / eMMC | 891 MB usable / 7.1 GiB |
| Display | 480x854 |
| OS on the phone | Android 7.0, build `091318` (2018-09-13) |
| Kernel | Linux 3.18.35, no loadable modules, no devtmpfs |
| Boot state | Bootloader unlocked |
| Boot partition | `mmcblk0p7`, 16 MiB, offset `0x1d80000` |
| Recovery partition | `mmcblk0p8`, 16 MiB, offset `0x2d80000` |

## Firmware

Stock firmware download:
<https://www.taiwanmobile.com/events/A32/A32UpgradeProcedure.html>

The kernel comes from the stock OTA package, which is not stored in git:
`phone-tools/firmware/full_esky6737m_35_n-ota-110520.zip` (build `110520`,
2020-11-05). Its whole-file signature verifies against the OTA certificate
installed on the phone. It is newer than the build on the phone.

## Build and test

```sh
cargo test-host          # runs unit tests on the Windows host
scripts/build-image.sh   # builds target/a32-boot.img, does not touch the phone
```

No ARM GCC toolchain is needed: the build uses Rust's bundled LLD and musl.
`gzip` and `unzip` come from Git Bash.

## Behavior

- Run as PID 1: mounts `/dev`, `/proc`, `/sys` and `/run`, creates device nodes,
  logs to the kernel log, starts a USB serial debug shell (VID `0e8d`, PID
  `2007`), draws a status screen with uptime and battery on `fb0` (RGBA8888),
  then reaps child processes forever.
- Run as any other PID: prints a read-only device report and exits.

Both paths have run on the phone. The PID 1 path ran from a RAM boot for 64
minutes on USB power with no reboot and no lost console, while the battery rose
from 71% to 84%. Running on battery alone has not been tested.

## Boot the image on the phone

The bootloader must be unlocked. This loads the image into RAM only, so a normal
reboot returns to Android:

```sh
fastboot boot target/a32-boot.img
```

About 20 seconds later the phone appears as a USB serial port (`0E8D:2007`,
`COM3` on the development PC). Open it with DTR enabled and type `help`.
`reboot bootloader` on the console returns to fastboot in about 12 seconds.

## Back up the boot partition

While the image is running, this copies the boot partition to the PC in about
75 seconds, using only the read-only `dump` console command:

```powershell
scripts/dump-partition.ps1 -OutFile phone-tools/backup/boot-mmcblk0p7.img
```

The serial link sometimes loses data, so each 64 KiB chunk is checked for length
and CRC-32 and requested again on failure. `cargo bootimg verify` checks the
image part of the result. Stock build `091318` of this phone is backed up in the
local, untracked `phone-tools/backup/`. Add `-Path /dev/recovery` for the recovery
partition.

## Flash to the boot partition

The phone now boots this system from its boot partition, and no longer starts
Android. Flash only when the console works in a RAM boot, because the console is
the way back to fastboot:

```sh
fastboot flash boot target/a32-boot.img
fastboot reboot
```

Pressing the power key on a powered-off phone without USB starts this system. The
battery level is an estimate that depends on how the phone started: it read 96%
and 97% after starting without USB, and 86% and 89% after starting with USB.

To return to Android, send `reboot bootloader` on the console, then flash the
backup of the stock boot partition:

```sh
fastboot flash boot phone-tools/backup/boot-mmcblk0p7.img
fastboot reboot
```

This round trip was tested: Android 2018-09-13 started from the backup in 67
seconds, and flashing this system again left the same image and stock recovery.

## Boot from flash

The bootloader accepts this unsigned image from flash. This was tested on the
recovery partition, leaving the boot partition untouched:

1. Run `fastboot flash recovery target/a32-boot.img`.
2. Run `fastboot boot target/a32-boot.img`, then send `reboot recovery` on the
   console. Do not go through Android, because of the next point.
3. Check that `/sys/class/BOOT/BOOT/boot/boot_mode` reads `2`.

The image also booted from recovery with USB unplugged before the restart
(`reboot recovery 30`). Unplugging USB right after the restart instead left the
phone powered off once; pressing the power key then started Android normally.

Android's `flash_recovery` service restores stock recovery on its next boot; the
result was byte-identical to the backup. The image therefore cannot stay in
recovery while Android is in use.

No key combination tried reaches fastboot from a powered-off phone: volume up,
volume down or both, with power. Fastboot has only been reached from software.

## Fastboot on the Windows development PC

- Fastboot mode only enumerates on a port of the AMD CPU USB controller
  (`VEN_1022&DEV_149C`). Ports on the chipset controller (`VEN_1022&DEV_43EE`)
  fail with "Device Descriptor Request Failed".
- The bootloader reports USB ID `0BB4:0C01` (product `ESKY6737M_35_N`), which is
  missing from `android_winusb.inf`. Install the Google USB Driver manually:
  Update driver, Browse my computer, Let me pick, Show All Devices, Have Disk,
  then choose Android Bootloader Interface.

## Known risks when booting the image

- The system does not start the stock battery services (`healthd`,
  `fuelgauged_static`). The kernel still charges the battery and reports its
  level and status, but charging to 100% and running on battery are unverified.
- Plugging in USB can stop charging: the kernel may read a low charger voltage at
  that moment (`Charger under voltage`) and keep reporting `Not charging`. This
  happened once in three replugs. Unplugging and replugging restored charging.
- The kernel in the image is newer than the build on the phone.

## Fail-safe

No key combination reaches fastboot, so the init reboots to fastboot by itself if
it panics, if the USB console setup fails, or if that setup has not finished after
60 seconds. A missing USB host is not a failure: the console became ready with the
USB state still `DISCONNECTED`, and the system kept running past the deadline after
USB was unplugged. `/run/console-ready` records that moment. A kernel panic is not
covered.

Each case was tested with a RAM boot, using an image built with a fault:

```sh
A32_FAULT=panic scripts/build-image.sh   # or usb, hang
```

Rebuild without `A32_FAULT` afterwards, and never flash a fault image.

## Power key

A short press turns the backlight off or on. Holding the key for 3 seconds powers
the phone off. Without USB it stays off until the key starts it again.

## Power off and charging

With USB attached, a powered-off phone starts by itself in charger mode. The USB
console does not appear in this mode, and unplugging USB turns the phone off. The
init therefore restarts into normal mode when `boot_mode` is `8`. It stays in place
for `9`, the low battery charger mode. The status screen shows the mode.

This was tested with `poweroff` while USB was attached. The phone started twice,
then ended in mode `0` with the console ready about 60 seconds later. Each start
briefly showed USB ID `0E8D:2000`, which is MediaTek's preloader.

As a result the phone cannot stay off with USB attached. `poweroff` only stays off
without USB.

To leave charger mode if the console is missing, unplug USB, which turns the phone
off, press the power key with USB unplugged, then plug USB back in.
