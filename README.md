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

## Kernel and storage facts

Read from the running phone. Other old devices should be checked the same way,
because searching the kernel image for strings gave wrong answers here: it did not
find FUSE, ext2 or ext3, which are all available.

| Item | Value |
|---|---|
| Filesystems | ext2, ext3, ext4, vfat, msdos, iso9660, FUSE, tmpfs. No overlay, squashfs or f2fs |
| Namespaces | `mnt` only, so containers such as Docker are not possible |
| Cgroups | `cpu`, `cpuacct`, `freezer` and `debug`. No memory controller |
| Missing | `/dev/net/tun`, `/dev/loop-control`, `/proc/config.gz`, DRM |
| Sleep states | `freeze`, `mem`, not tried |
| CPU | ARMv7 mode with NEON, VFPv4, integer divide and SHA-2. At most 1092 MHz |
| CPU cores | One was online when idle, `cpu0`. How others come online was not checked |
| CPU governor | `interactive`. Also `ondemand`, `userspace`, `powersave`, `performance` |
| Listening sockets | None on TCP, TCP6 or UDP, measured after Wi-Fi joined a network |

Partition sizes in MiB: `userdata` 3631, `system` 3072, `cache` 400, `metadata` 37,
`nvdata` 32, `boot` 16, `recovery` 16. `nvdata` holds the Wi-Fi MAC address and
calibration, and `nvram` probably holds similar data, so nothing should write to them.

## Storage

`userdata` is formatted as ext4 and mounted on `/data`, after `e2fsck -p`. This erased
Android's data. Formatting is a manual step, and the init never does it, so a passing
fault cannot erase `/data`. If the mount fails, the system runs and saves nothing.

Back up the small partitions before writing to anything. This took about 11 minutes
for 18 partitions, and works from the phone's own partition table. `proinfo`, `nvram`
and `nvdata` were then read a second time and matched:

```powershell
scripts/backup-partitions.ps1
```

Then, on the console, after checking the partition's name and size in
`/sys/class/block`:

```text
sh mke2fs -t ext4 -L a32data DEVICE
```

Power cuts while writing to `/data` have not been tested.

## Performance

The `bench` console command measures the phone, and this saves the results to
`benchmarks/amazing-a32/LABEL.txt`:

```powershell
scripts/bench.ps1 -Label baseline -Runs 3 -Net -LoadSeconds 300
```

Baseline from a RAM boot with USB attached, built with `opt-level = 3` and no
`target-cpu` setting. Values are the range over 3 runs:

| Measurement | 1 thread | 4 threads |
|---|---|---|
| Integer (CRC-32), MB/s | 66.2 to 66.6 | 250.7 to 254.2 |
| Floating point (matrix), Mflop/s | 115.7 | 427.1 to 432.7 |
| Vectorizable integer, MB/s | 1552 to 1560 | 5883 to 5926 |

| Measurement | Result |
|---|---|
| Memory copy / fill | 1235 to 1270 / 3719 to 3720 MB/s |
| Memory latency, 16 KiB / 256 KiB / 2 MiB / 64 MiB | 5.6 / 18 / 176 / 219 ns |
| eMMC sequential read | 73.6 to 74.4 MB/s |
| eMMC random 4 KiB read | 2492 to 2520 IOPS |
| Wi-Fi TCP, phone receiving | 44.8 to 46.2 Mbit/s |
| Wi-Fi TCP, phone sending | 42.6 and 43.7 Mbit/s, and 22.8 once |

`mem.read` is left out, because it only reads every fourth byte. The Wi-Fi figures
are for a computer on Ethernet behind the same router, on 2.4 GHz at about -32 dBm.

A 5 minute load on all CPUs held 242 to 246 MB/s at 1092 MHz with all 4 CPUs online,
so no slowdown appeared. The CPU rose from 47.1 to 51.9 C and the battery from 32 to
34 C, and both were still rising slowly at the end, so this does not show the
temperature it settles at. `bench load` stops at 85 C on the CPU or 45 C on the
battery, because of the next point.

All thermal zones read `disabled`, presumably because the stock service that sets them
up does not run. Do not assume the kernel slows the CPU when it gets hot, so the init
has its own thermal service.

eMMC writes go to a new file on `/data`, past the page cache, and vary much more than
reads. `bench write` runs for 5 seconds per test. Over 9 runs, sequential writes were
about 16 MB/s in 4, with 0.4, 7.3, 12.7, 14.7 and 23.9 MB/s in the others. Random 4 KiB
writes were 2255 to 2574 IOPS in 7, and 1224 and 1312 in the others. Disk counters
showed no other writer in the 6 runs where they were recorded, so the eMMC's own
housekeeping is a possible cause, but this was not proven. A service should not count
on steady write speed.

The thermal service caps the CPU at 637 MHz at 80 C and releases it at 70 C. An image
built with `A32_CPU_CAP_C=50`, for testing only, capped at 50.6 C under load: the
speed fell from 245 to 142 MB/s, the CPU settled near 45 C, and it released at 39.8 C.
At 95 C on the CPU or 55 C on the battery the phone powers off. That has not been
tested, and neither has the cap at its real limit. With USB attached the phone starts
again by itself after powering off.

Not measured: USB networking, power use, and running on battery.

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
  level and status. It reached `100` and `Full` on USB power, but the level is an
  estimate: it read `94` and `Charging` after two later restarts within minutes.
  Running on battery is unverified.
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

## Touch screen

The status screen draws a cross at the touched position and shows its coordinates.
The top left corner read `4,4` and the bottom right read `476,850`, so the
coordinates match the 480x854 display without calibration.

Open problems:

- The cross lags behind the finger. The cause has not been measured.
- `/dev/touch` was missing on one boot. Six later boots were fine, and each found
  the device at once, so the retry added since then is not proven to be the fix.
  `/run/init.log` now keeps the reason if it happens again.

## Wi-Fi

The init starts Wi-Fi by itself, using the stock programs and firmware from the
read-only `/system`, and joins the saved network if there is one. `/run/wifi-state`
reads `starting`, `ready`, `failed: REASON` or `off: low battery`. A failure leaves
the console running, and low battery charger mode skips Wi-Fi.

To join a WPA2 network, and keep it for later starts, on the console:

```text
wifi join SSID PASSPHRASE
wifi
wifi forget
```

Use double quotes around a value with spaces. The details are saved only after
joining works, so a mistyped passphrase does not replace a good one, but a failed
join does leave the phone off the network until the next restart.

**The passphrase is stored unencrypted** in `/data/a32/wifi-network`, readable only by
root. Anyone who can reach the console or read the eMMC can get it. It is not put in
the image, not sent through a shell and not logged. `wifi forget` removes the file,
but flash storage may keep old data. The console echoes what is typed back to the
connected computer.

While connected, the init renews the lease halfway through, and gets a new one after
a lost connection. The address is left alone if it did not change, so open
connections survive. `/etc/resolv.conf` follows the DNS server of the lease. The
clock is set by SNTP after connecting and every 6 hours, and saved in the RTC.

Test results, all from RAM boots restarted through fastboot with USB attached:

- 10 of 10 boots with a saved network joined about 28.5 seconds after the kernel
  started, with the clock set about 0.4 seconds later. All had the same MAC and IP
  addresses, `/data` mounted and `/dev/touch`.
- After deleting the calibration copy, a restart restored it from `nvdata`.
- 12 of 12 pings were answered during a lease renewal, with the same address.
- After `wpa_cli disconnect` then `reconnect`, the phone got a lease and reached
  8.8.8.8 again. This is not the same as losing the access point, which is untested.
- A wrong passphrase left the saved file unchanged, and was not found in `/run`,
  `/data` or the kernel log. After `wifi forget` a restart stayed off the network.
- A name resolved and TCP connected to it. The clock read within about 2 seconds of
  the PC, which is about the delay between the two readings.
- 11 of 11 earlier boots reached `ready` in 11 seconds, on the first power on attempt.
  Each one then scanned and found 14 to 21 networks, and had `/dev/touch`.
- `wifi join` with a network that does not exist, tried twice, reported the failure
  and left no network configured. The passphrase was not found in `/run` or in the
  kernel log.

To repeat the boot tests, which do not write to flash. A saved network is rejoined:

```powershell
scripts/test-wifi-boots.ps1 -Count 10 -JoinFailure
```

- `wifi join` joined a 2.4 GHz WPA2 guest network and got an address in 9 seconds,
  without a restart after leaving another network. The gateway, 8.8.8.8, 1.1.1.1
  and 168.95.1.1 each answered 6 of 6 pings, and TCP connected to the three public
  addresses. This was one join, and passphrases were not found in `/run` or in the
  kernel log.
- `wifi join` also joined a device's hotspot, shared from its own Wi-Fi, in 10
  seconds, but the phone could not reach beyond the gateway there, although another
  device reported internet access through it. The public addresses answered no pings
  and TCP to them timed out, while TCP to the gateway connected. A capture on the
  phone showed the pings to 8.8.8.8 leaving with the right addresses, to the
  gateway's MAC address, with no reply. Since the guest network worked in the same
  session, the phone's routing is not the likely cause, but the actual cause is not
  known. Other devices may meet the same hotspot behavior.

`tcp HOST:PORT` on the console tries a TCP connection, because the stock system has
no `nc`. `tcpdump` is in `/system/xbin`.

Not tested:

- A start without USB, and running on battery with Wi-Fi on.
- A renewal at its real time, which is 36 hours away on this network. It was tested
  through the `dhcp` command, which runs the same code.
- HTTPS, because the phone has no TLS client yet. Only the clock it needs was checked.
- Running for longer than 2 hours.

To record a network over time, which never joins or restarts anything:

```powershell
scripts/test-wifi-soak.ps1 -Hours 2
```

A 2 hour run on the guest network, at about -32 dBm with USB attached, gave 120 of
120 checks with no restart, no lost console and no lost connection. 720 of 720 pings
were answered, half to the gateway and half to 8.8.8.8, whose average round trip was
8 ms and at most 22.5 ms per check. 120 of 120 TCP connections opened. Free memory
stayed between 835368 and 837508 kB.

The init runs these steps, in this order:

1. Point the kernel at the firmware: write `/system/vendor/firmware` to
   `/sys/module/firmware_class/parameters/path`. Whether this is required has not
   been checked.
2. Create `/dev/wmtdetect`, `/dev/btif` and `/dev/ttyMT2` from `/sys/dev/char`.
3. Run `wmt_loader`, then create `/dev/stpwmt` and `/dev/wmtWifi` the same way.
4. Run `wmt_launcher -p /vendor/firmware/`, then write `1` to `/dev/wmtWifi`.
   `wlan0` appears.
5. Run `ip link set wlan0 up`, then `wpa_supplicant`.

`wifi join` then adds the network with `wpa_cli` and gets an address. The stock
system has no DHCP client program, so this one is written in Rust. `wmt_loader` ends
with status 255 on every boot seen, and Wi-Fi works anyway. The reason is not known.

Requirements found on the way, which other devices may share:

- `wmt_launcher` waits for the property `service.wcn.driver.ready`, normally set
  through a property service. Without it the kernel reports `no hif info!`. The
  init provides it in a read-only `/dev/__properties__`, in the Android 7.0 format.
- Android's `wpa_supplicant` drops to uid 1010, then exits with status 255 and no
  output if it cannot create its control socket. Its directory and
  `/data/misc/wifi/sockets` must belong to that user.

- The Wi-Fi driver reads its MAC address and calibration from
  `/data/nvram/APCFG/APRDEB/WIFI`, with the MAC address at offset 4. Without that file
  it makes up an address on each start. The stock system restores the file from the
  `nvdata` partition, and the init copies it once, mounting `nvdata` read-only.
- Setting the address with `ip link set wlan0 address` after power on did not change
  it. Whether this failed or the driver replaced it was not determined.

The `sh LINE` console command runs a line with the stock shell.

## Power off and charging

With USB attached, a powered-off phone starts by itself in charger mode. The USB
console does not appear in this mode, and unplugging USB turns the phone off. The
init therefore restarts into normal mode when `boot_mode` is `8`. It stays in place
for `9`, the low battery charger mode. The status screen shows the mode.

This was tested with `poweroff` while USB was attached. The phone started twice,
then ended in mode `0` with the console ready about 60 seconds later. Each start
briefly showed USB ID `0E8D:2000`, which is MediaTek's preloader. With this system in
the boot partition, 3 of 3 tries returned to mode `0` in about 60 seconds with Wi-Fi
`ready`.

As a result the phone cannot stay off with USB attached. `poweroff` only stays off
without USB.

To leave charger mode if the console is missing, unplug USB, which turns the phone
off, press the power key with USB unplugged, then plug USB back in.
