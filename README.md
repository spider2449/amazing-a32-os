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
| Missing | `/proc/config.gz`, DRM, the hung task and soft lockup detectors |
| Not created yet | `tun` and `loop-control` are registered misc devices, listed in `/sys/class/misc`, but the init does not make their `/dev` nodes. An earlier check missed them for that reason. Neither has been tried |
| Watchdogs | The kernel kicks the hardware watchdog every 20 s from its own threads. MediaTek's hang detector, `/dev/RT_Monitor`, only reports. See Watchdog |
| Sleep states | `freeze`, `mem`, not tried |
| CPU | ARMv7 mode with NEON, VFPv4, integer divide and SHA-2. At most 1092 MHz |
| CPU cores | Only `cpu0` is online when idle. MediaTek's hotplug (`/proc/hps`, `enabled` 1, `num_base_perf_serv` 1, `up_threshold` 95) brings the others online under load: with 4 busy loops, `/sys/devices/system/cpu/online` read `0-3` within 1 s. `htop` shows the idle ones as `offline` |
| CPU governor | `interactive`. Also `ondemand`, `userspace`, `powersave`, `performance` |
| Listening sockets | None on TCP, TCP6 or UDP, measured after Wi-Fi joined a network |
| Bluetooth | No kernel Bluetooth stack: no `/sys/class/bluetooth` and no `vhci`, so BlueZ cannot run. MediaTek's `/dev/stpbt` takes HCI commands directly. See Bluetooth |
| Load average | About 6 when idle, because 6 MediaTek kernel threads, such as `hang_detect` and `bat_thread_kthr`, sleep in the uninterruptible state, which Linux counts as load. CPU use was about 1% at the same time, so the load average says nothing here |

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

One power cut was tested, from the phone booted from flash: with 64 MB files being
rewritten on `/data` and a counter file appended and synced in a loop, USB was
unplugged and the battery pulled out about 10 seconds later. After the battery went
back in and the power key started the phone:

- `e2fsck` reported `recovering journal`, then `clean`. `/data` mounted, and the
  kernel logged no ext4 errors.
- The saved network was intact: Wi-Fi rejoined it at 32 seconds. SSH started, and
  `apk audit --system` in Alpine found nothing changed.
- The counter kept every synced line up to 7, written about 25 seconds before the cut.
  Under the large writes each `sync` took about 20 seconds, so later lines were not yet
  on the eMMC, as expected. The persistent log kept a line from about 10 seconds
  before the cut.
- The RTC lost the time with the battery out: it restarted at `2010-01-01`, so log lines
  read 2010 until the network set the clock. The uptime in each line still orders them.
  This phone's RTC has no backup cell, which may be true of other phones too.
- `/proc/aed/reboot-reason` read `WDT status: 0` after the power cut, against `2` after
  software restarts.

This is one cut, so it shows the journal works, not that every cut is harmless.

## Alpine Linux and SSH

Alpine runs in a chroot on `/data/alpine`, and the Rust init stays PID 1 outside it. So
the console and the fail-safe do not depend on Alpine. `alpine LINE` on the console runs
a line inside it. The init starts `sshd` once it is installed, and restarts it if it ends.

To install, with the phone on a network this computer can reach:

```powershell
ssh-keygen -t ed25519 -f "$env:USERPROFILE\.ssh\a32"
scripts/install-alpine.ps1 -Archive alpine-minirootfs-VERSION-armv7.tar.gz
ssh -i "$env:USERPROFILE\.ssh\a32" root@PHONE_ADDRESS
```

First check the archive's SHA-256 and its GPG signature from alpinelinux.org. The script
unpacks it on the computer, because the stock `gzip` could not decompress and `tar` has
no `gunzip` to call. It sends the `.tar` with `recv`, which has no authentication, then
reads the file back over the serial console and compares the hash before unpacking.

The init names the host `a32`, so shell prompts no longer show the kernel's `(none)`;
`hostname` read `a32` on a RAM boot and after flashing. SSH takes only keys for root. Test results, from RAM boots of Alpine 3.24.3:

- A listed key logged in and got a terminal. An unlisted key, a removed key and a
  password attempt were all refused.
- HTTPS works: `apk` installed packages. With `curl`, a valid certificate verified. An
  expired one and a wrong host name were refused, by certificate error. Other
  `badssl.com` pages timed out or reset, so those results say nothing.
- After `sshd` was killed, it answered again 6 seconds later. On 3 of 3 restarts it
  started about 15 seconds after the kernel, before Wi-Fi joined at about 28.6 seconds.

Known limits:

- `/proc/mounts` lists paths from outside the chroot, so `df` prints errors for `/data`
  and `/system`, and `df /` shows no size. Run `sh df -h /data` on the console instead.
- `reboot` inside Alpine asks PID 1, which does not listen. `reboot -f` should work, but
  this has not been tried.
- OpenRC services do not run. Only `sshd` is started.

## Firewall

The init sets the firewall with the stock `iptables` before Wi-Fi starts. Incoming traffic
is dropped, except replies to connections from the phone, ping, DHCP replies, and SSH and the
status page from private addresses (`10.0.0.0/8`, `172.16.0.0/12`, `192.168.0.0/16`). All
incoming IPv6 is dropped. Neither `sshd` nor the status page starts unless all rules were
applied. `recv` and `bench net` open
their port only while they run. The `conntrack` match is missing here, but `state` works.

From the PC, ping worked and a closed port did not connect. Refusing a public source
address has not been tested, because that needs a test from outside the local network.

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

Not measured: USB networking and power use. Running on battery will not be tested, because the phone runs on USB power as a server.

### Tuning

`scripts/tune-sweep.ps1` compares runtime settings and records them in
`benchmarks/amazing-a32/`. A setting stays only if it measurably helps.

| Setting | Result | Decision |
|---|---|---|
| `io_is_busy=1` on the `interactive` governor | Random reads 1771 to 3485 IOPS, random writes 1712 to 3725, sequential reads 67 to 94 MB/s. Idle CPU temperature within 0.4 C | Kept |
| Screen off after 60 s, with no drawing | Idle CPU 12.2% to about 1.4% over 8 windows. One earlier window read 5.1%, not explained and not seen again | Kept |
| Screen wake, checked by hand | A touch turned it on, it dimmed 60 s after the last use, and the power key turned it on and off | Works |
| Reading `operstate` in place of running `wpa_cli` every 5 s | Part of the same idle result, not measured alone. It read `up`, `down`, `up` through a disconnect | Kept |
| `performance` governor | No faster than `io_is_busy=1` | Not used |
| Building for `cortex-a53` with NEON | CPU results within run-to-run variation | Not used |
| I/O scheduler `deadline` or `noop` | No clear difference from `cfq` | Not used |
| All 4 CPUs always online | No clear difference | Not used |
| zram, 445 MiB of swap with `lzo` | Made things worse. A 1100 MB load finished 0 operations, spending about 25 s in the kernel, with only 1 to 2 MB swapped. The cause is not known | Not used |

The `io_is_busy` figures come from 3 rounds with the settings in turn, because eMMC writes
vary with time. The earlier baseline of about 2500 IOPS was raised by the screen drawing,
which kept the CPU awake. Stopping the drawing alone would have lowered it to about 1771,
so these two changes belong together. Sequential writes still vary widely, from 0.2 to
26 MB/s, under every setting.

Without swap, a 1100 MB load made the kernel kill only that program, 9 times. The console,
SSH and Wi-Fi survived, and the phone did not restart. Other programs may not be so lucky,
because all of them have the same priority for being killed.

Wi-Fi is limited by the link. It connects at up to 65 Mbit/s, on 2.4 GHz with one stream
and a 20 MHz channel, so 45 Mbit/s of TCP leaves little to gain in software. Results also
depend on the time of day: an evening test gave 30 to 39 Mbit/s receiving and 8 to 21
sending, with the link rate often at 39 and a strong signal of about -35 dBm, whether
`io_is_busy` was on or off. Busy airwaves are a likely cause, but this was not proven.

After tuning, compared with the baseline: integer and floating point on 4 CPUs rose about
3%, which may only be the reduced idle work, and single CPU results did not change.

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
  `2007`), draws the touch panel on `fb0` (RGBA8888), runs the services in the
  sections below, then reaps child processes forever.
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
  happened once in three replugs. Unplugging and replugging restored charging. The init now logs it and shows it on the panel.
- The kernel in the image is newer than the build on the phone.

## Fail-safe

No key combination reaches fastboot, so the init reboots to fastboot by itself if
it panics in its first 10 minutes, if the USB console setup fails, or if that setup
has not finished after 60 seconds. A later panic restarts normally, as described
under Watchdog. A missing USB host is not a failure: the console became ready with
the USB state still `DISCONNECTED`, and the system kept running past the deadline
after USB was unplugged. `/run/console-ready` records that moment. The kernel is set
to restart 1 second after a kernel panic (`panic=1`, `panic_on_oops=1`), which has
not been tested.

Each case was tested with a RAM boot, using an image built with a fault:

```sh
A32_FAULT=panic scripts/build-image.sh   # or usb, hang
```

Rebuild without `A32_FAULT` afterwards, and never flash a fault image.

## Watchdog

The init restarts the phone normally, which is the only repair an unattended phone
can make for itself, when:

- A loop stops reporting progress: 180 seconds for the reaper, the thermal service and
  the panel, and 600 seconds for the network loop, whose round can include DHCP, DNS
  and three time servers. A loop that ends on purpose, such as the panel without a
  display, is no longer watched.
- A network is saved and has not worked, meaning no lease or `wlan0` not up, for 30
  minutes. After each such restart in a row the wait doubles, up to 24 hours, so a
  router that is gone for days does not restart the phone every half hour. The count
  is kept in `/data/a32/network-restarts` and removed once the network works. Low
  battery mode, which leaves Wi-Fi off, is not counted.
- The init panics after running for 10 minutes. Such an image booted and started its
  console, so the panic is treated as a passing fault, not a broken image.

A restart first waits at most 15 seconds for cached writes, so that a stuck eMMC
cannot stop the restart that may repair it, and falls back to `sysrq` if the restart
call fails. The console and the SSH service are not watched: both wait on input or a
child for as long as it takes, so a missing heartbeat would not mean a fault.

What the kernel offers, read on this phone:

- The hardware watchdog is kicked by the kernel's `wdtk` threads every 20 seconds
  (`/proc/wdk`), so it only catches a hung kernel. There is no `/dev/watchdog`.
- MediaTek's hang detector, `/dev/RT_Monitor` (misc 10:57), is the watchdog Android's
  `system_server` uses. It was armed with ioctl `0x8004700A`, whose argument is
  seconds, counted in 30 second steps. It disarms itself unless a process named
  `system_server` exists. With such a process it counted down, then at zero only
  dumped thread stacks every 30 seconds ("we should triger Kernel API DB", a report
  for Android's crash service). It was still counting at -9, about 5 minutes past
  zero, with no restart, so it is not used. Other MediaTek phones may behave the same.
- The kernel has no hung task or soft lockup detector.
- `/proc/aed/reboot-reason` starts with a line such as `WDT status: 2 fiq step: 0
  exception type: 0`, which the init logs at start as the previous reset. It read
  `WDT status: 2` after a `reboot bootloader`, so 2 does not mean a watchdog reset.
  It read `0` after a power cut. The other values have not been seen.

Test results, from RAM boots of images built for each test, restarting into this
system from the boot partition:

| Test | Result |
|---|---|
| `A32_FAULT=stall`: the thermal loop stops after its first round | Restarted at 190 s of uptime: `thermal made no progress for 180 s` |
| `A32_FAULT=late-panic A32_STABLE_S=60`: a panic at 100 s | Restarted normally, not to fastboot, and logged the panic message |
| `A32_NETWORK_LIMIT_S=60`, then `wpa_cli disconnect` | Lost at 39 s, restarted at 100 s, count `1`. The next session joined and removed the count |

The normal image then ran 450 seconds with no restart, and used 0.97% of the CPU over
5 minutes with the screen off, about the same as before the watchdog.

The doubling of the network wait is only unit tested, because it needs the saved
network to stay unreachable across restarts. A stuck eMMC and a failed restart call
have not been tested.

```sh
A32_FAULT=stall scripts/build-image.sh                        # or late-panic, with A32_STABLE_S=60
A32_NETWORK_LIMIT_S=60 scripts/build-image.sh
```

Never flash these images. `scripts/ram-boot.ps1 -Image IMAGE` boots one from RAM.

## Logs

The init logs to the kernel log, to `/run/init.log`, and, once `/data` is mounted, to
`/data/a32/log/init.log`, which survives restarts. Each session there starts with
`--- session start ---` and the lines logged before `/data` was mounted. Lines carry
the UTC time and the uptime. The time comes from the RTC until the network sets the
clock, and was right in the tests. At 1 MiB the file moves to `init.log.1`, replacing
the older one. Lines are not flushed one by one, so a power cut can lose the last few
seconds. No passphrase is logged. To read it on the console:

```text
sh tail -n 50 /data/a32/log/init.log
```

`scripts/console.ps1 -Lines LINE, LINE` sends console lines from the computer and prints
what each returned.

## Status page

`http://PHONE_ADDRESS/` shows what the panel's home page shows, and the last 40 lines of
the persistent log. It reloads itself every 15 seconds. `/status.json` has the same
values for scripts, and `/bluetooth.json` lists the devices the BLE scan heard, with
full addresses. These only read: they take nothing but the path, so they have no
login, and the firewall only lets private addresses reach port 80. The cameras
(`/camera/...`, see [Cameras](#cameras)) need the web password. It is part of the
init, with no web server program.

Requests are served one at a time, so that no client can use up the phone's memory or
threads; only a live camera view gets its own thread, and a second view is refused
at once. A client that connects and sends nothing therefore delays the others by up
to 5 seconds, its time limit. Requests over 4 KiB get `431`, other methods than `GET` get
`405`, and other paths `404`.

Tested from the PC on a RAM boot: the page, the JSON, `404` for `/nope` and
`/../etc/passwd`, `405` for `POST`, `431` for 6000 bytes of headers, and a request
answered 5 seconds after a silent connection. The page was also checked in a browser.
The SSID and log lines are escaped, so a network name cannot add HTML to the page.

## Charger and battery monitor

The stock battery services do not run, so the init checks the charger and the battery
once a minute and logs:

- A line every 15 minutes with the level, the battery voltage, the status and the
  charger voltage, which records how long the battery lasts.
- The charger being attached or removed.
- Charging stopped for 5 minutes with the charger attached, the problem under Known
  risks, and charging again afterwards. While stopped, the panel's battery level reads
  `NOT CHARGING`. No software way to restart charging is known, so the log asks for USB
  to be plugged in again.

On battery at 3% or less, or under 3400 mV, for two checks in a row, it powers off
cleanly, so that the voltage cannot fall far enough to cut power during a write. The
percentage is an estimate, so either limit is enough. This has not been tested, and
3400 mV is a common cutoff for this kind of battery, not one measured on this phone.
It will stay untested, because the phone runs on USB power and battery tests were
dropped.

For its first moments, about 10 seconds after the kernel starts, the battery driver
reads 0 mV, 50% and no charger. Such readings are ignored, since 0 mV would otherwise
count as an empty battery. Other devices may have the same delay.

Test, from a RAM boot with the screen off: USB was unplugged for 2 minutes and then
plugged in again. Both were logged within the minute, with the status `Not charging`
while unplugged (not `Discharging`) and the charger at 18 mV. The phone kept running
and stayed on the network, and charging resumed after the replug. That is one replug,
so it does not rule out the problem under Known risks.

## Power key

A short press turns the backlight off or on. Holding the key for 3 seconds powers
the phone off. Without USB it stays off until the key starts it again.

## Touch panel

The screen is a panel, drawn by the init with no other graphics software. The home page
shows the address, Wi-Fi, SSH, CPU, memory, disk and battery. Its other pages restart or
power off the phone, scan for and join a WPA2 network with an on-screen keyboard, and turn
the BLE scan on or off, take photos, show live video, and give a Linux shell. The Bluetooth page shows the
scan's state, up to 4 sensors with their values, and the 6 strongest devices. Its ON and OFF buttons act at once, without asking,
because neither loses anything, and the page notes that scanning slows Wi-Fi.
Restarting, powering off and forgetting the network each ask first, with the second button
in a different place, so a double tap cannot confirm by accident.

A button acts when the finger lifts over it, and only if the finger came down on it. The
screen turns off 60 seconds after the last use and drops anything typed. A touch or the
power key turns it on, and that first touch presses nothing. Holding the power key for 3
seconds powers off, as before. The RESTART and POWER OFF buttons were each tested once with
`ui tap`, with USB attached: the phone left and returned, and after a power off it rejoined
the network.

The home page's buttons are WI-FI, BLE, CAMERA, CONSOLE and POWER, in small letters
so that five fit, and DESKTOP beside the title, for the X11 desktop (see Desktop). CONSOLE is a terminal with Alpine's login shell (`/bin/sh -l` in the
chroot, as root, `TERM=vt100`) on a pseudo-terminal, 39 columns wide. The init's own
commands stay on the USB serial console. The page has a small terminal emulator
(`src/terminal.rs`) that handles what a shell and line-based programs use: CR, LF,
backspace, tab, cursor moves and erasing. For full-screen programs it also handles
scrolling regions, reverse index, inserting and deleting lines, reverse video (drawn
as dark text on a light bar), saving the cursor, hiding the cursor, application
cursor keys (the arrows then send `ESC O`) and the alternate screen, which has no
scrollback and gives back the shell's screen on exit. Other colors and attributes are
dropped, and other characters than ASCII show as `?`. Checked on a RAM boot by typing
into the console's shell with `TIOCSTI` and taking screenshots: BusyBox `vi` opened a
60-line file, jumped to line 50, scrolled back up with 40 `k`, and `:q!` left the
shell's screen as before; `less` showed `(END)` in reverse video and scrolled back 5
lines; `top` drew its header bar in reverse video. BusyBox's `vi`, `less` and `top`
only move the cursor, erase lines and set reverse video, so the other sequences are
tested only by unit tests so far. After flashing, `htop` 3.5.3 (ncurses) drew its
meters, its reverse-video header and selection bar, and its process list; at 39
columns it shows only the first columns of the list. Dragging the text with a finger scrolls back through the last 3000 lines,
and the header says how far; typing returns to the end. The keyboard has six rows
48 px high, laid out like a phone keyboard: CTRL (for the next key, so CTRL then `c`
is Ctrl-C), ESC, TAB and the arrows; the digits; three rows of letters, the last
between SHIFT and DEL; and 123, `,`, SPACE, `.` and ENTER. SHIFT, like CTRL, applies
to the next key only. 123 shows one layer with every other printable character. The
Wi-Fi passphrase keyboard has the same rows, without the top row, and its symbol
layer also holds `,` and `.`. Checked on a RAM boot with `ui tap` and screenshots:
digits, `,`, `.`, a capital, a symbol, DEL, one-shot SHIFT, and both keyboards
drawn. Then flashed: the phone booted from it and the console showed the new keyboard. HIDE KEYS shows 44 lines instead of 28 and tells the shell the new size. The shell starts when the page first
opens and keeps running when the page or the screen closes, so earlier output stays
for scrolling back. After `exit` the page says so, and the next key starts a new
shell. Output does not keep the screen on. Nothing typed is logged. Anyone holding
the phone can run commands as root, as the USB serial console already allows.
Checked on a RAM boot with `ui tap`, `ui drag` and screenshots: `ls /`, `dmesg`
scrolled back 16 lines by a drag, the keyboard hidden, the up arrow bringing back
`dmesg`, and CTRL then `c` cancelling it. After flashing, the page ran `ls` in a new shell.

The list shows the 8 strongest WPA2 personal networks, with no way to scroll. Open and
enterprise networks are not listed, because this system cannot join them. Text is ASCII, so
other characters in an SSID show as `?`, but such a network can still be joined.

The Bluetooth page was checked on a RAM boot with a screenshot, showing the scan, the
LYWSDCGQ at 26.00 C and 72.20%, and the strongest devices. Its OFF and ON buttons each
worked once, pressed while someone used the panel by hand at the same time. A screenshot
takes about 7 seconds while the panel redraws each second, so it can mix rows from two
frames: one showed a device twice, while `/bluetooth.json` had no repeated address.

Touch coordinates match the 480x854 display without calibration: the top left corner read
`4,4` and the bottom right `476,850`.

To check the panel from the computer, `ui tap X Y` on the console acts like a finger,
`ui drag X1 Y1 X2 Y2` moves one, and this saves the screen. A saved screen can show a passphrase, so delete it after use:

```powershell
scripts/screenshot.ps1 -OutFile target/screen.png
```

Drawing used to be slow enough to see: 5.3 frames a second. `bench display` showed the time
was in writing the frame with `write`, at 153 ms of about 188 ms. Mapping the framebuffer and
copying only the rows that changed gave about 55 frames a second, with the panel drawing as
fast as 2.8 ms. Other old devices may have the same problem.

`/dev/touch` was missing on one early boot. Later boots found it each time, and
`/run/init.log` keeps the reason if it happens again.

## Desktop

The panel's DESKTOP page starts an X11 desktop: the X server on `fb0` with the JWM window
manager, from Alpine. It needs these packages, and the page names any that are missing:

```text
apk add xorg-server xf86-video-fbdev xf86-input-evdev jwm xterm font-dejavu
```

With them and a few test tools, `apk` reported 267 MiB for all of Alpine. The X server
took 20 to 23 MB of memory, JWM 6 MB and each xterm 5 MB, and the idle CPU use stayed
near 0%.

Found on this phone, and worth checking the same way on other old devices:

- The kernel has no virtual terminals (no `tty0`), yet the X server starts with
  `-novtswitch -sharevts`. Its `fbdev` driver draws into `fb0`, and the screen showed each
  change, such as a clock ticking every second, without the pan the panel uses.
- The touch screen reports the old multi-touch protocol A: `SYN_MT_REPORT`, no tracking
  ids, and never the `ABS_X` and `ABS_Y` it lists. The X server's `evdev` driver got no
  events from it at all, so the pointer did not move. Raw events read correctly with
  `evtest`, so the event layout was not the cause.
- The kernel has `uinput`. So the init reads the touches, as it does for the panel, and
  moves a mouse it makes with `uinput`, `/dev/input/a32-pointer`, with a left and a right
  button and axes as large as the screen. The X server is given only that device.

Touches become mouse events in `src/input.rs`. A tap clicks the left button where the
finger was. A finger that moves more than 20 pixels drags with the left button from where
it came down; a held finger drifted about 6 pixels. A finger held still for 1.5 seconds
clicks the right button. The user tried 0.7, 1, 2 and 1.5 seconds by hand and chose 1.5.

Alpine's JWM settings open the desktop's menu with the left button too, so a drag that
started on the empty desktop opened the menu first. The init therefore starts JWM with a
copy, `/run/a32-jwmrc`, where that menu and the JWM button use the right button. A
`/root/.jwmrc` in Alpine is used instead, as it is.

The panel stops drawing while the desktop runs, and touches go only to the desktop. The
power key and the screen timeout work as on the panel. The desktop ends when JWM or the
X server ends, for example with Exit in the JWM menu; the init then ends the other one,
draws the whole panel again, and shows why on the DESKTOP page. The X server's log is in
`/run/xorg.log`, and the output of both programs in `/run/desktop.log`. Alpine's `/tmp`
is on `/data`, so the server's lock and socket from before a restart stay there; the init
removes them before starting, once it has found that nothing listens on them.

Checked on RAM boots, with `ui tap` and screenshots, and by hand: the JWM menu opened a
terminal; the user dragged windows, switched desktops and opened the right-button menu;
Exit with OK returned to the panel with the X server, JWM and xterm ended; killing the X
server returned to the panel too; and a second start worked. `ui drag` cannot move a
window, because it sends all of its points at once, and JWM reads the pointer's position
when the button goes down, by which time the pointer is already at the end. There is no
on-screen keyboard on the desktop yet.

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
  8.8.8.8 again.
- Restarting the router, which was down about 2 minutes, from the phone booted from
  flash: the connection was lost at 284 s and back at 404 s with no restart. The first
  lease then came from `192.168.0.254` and gave `192.168.0.188`, a different network
  from the usual `192.168.1.1`. 10 seconds later the connection dropped and came back
  with the usual address, and 8.8.8.8 answered. The access point is a TP-Link in bridge
  mode, and its owner believes its old router settings served that lease while it was
  starting. That fits, but neither `wpa_supplicant` nor the kernel log kept which
  access point gave it. Each new connection got a new lease, which is what made the
  phone follow the change. Access points that serve their own DHCP while starting may
  do the same to other devices.
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
- Running for longer than 2 hours. A planned 7 day run was dropped in favor of new features.

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

## Bluetooth

The kernel has no Bluetooth stack, so BlueZ and `bluetoothctl` cannot run. The radio is
still usable through `/dev/stpbt` (major 192), MediaTek's character device, which takes
HCI commands as H4 packets, the way Android's Bluetooth library uses it. Opening it turns
Bluetooth on through the WMT driver, and closing it turns it off. The init already
creates the device as part of the Wi-Fi start.

Test results, from a RAM boot with Wi-Fi connected:

- HCI reset worked. The chip reports HCI version 8, which is Bluetooth 4.2, from
  manufacturer `0x0046`, MediaTek.
- The Bluetooth address read `00:00:46:03:35:01`, which looks like a default. Android
  sets the real one from NVRAM, and where this phone keeps it has not been checked.
- A passive BLE scan of 8 seconds received 265 and 266 advertisements from 21 and 22
  devices, in two runs, the strongest at -65 dBm.
- Wi-Fi stayed connected while Bluetooth was on.

Not tested: sending, such as advertising or connecting to a device, and classic
Bluetooth.

### BLE scan

The init can keep a BLE scan running. It is active, as a phone's scan is: it asks each
advertiser for its scan response, which is where most devices put their name. It never
advertises or connects. It is off unless chosen, on the panel or the console, and the
choice is kept in `/data/a32/bluetooth` across restarts:

```text
bluetooth on
bluetooth
bluetooth off
```

`bluetooth` lists the 20 strongest of the devices heard in the last 10 minutes, with
the address, its type, the signal, when it was last heard, and the name if it sends one,
or else what kind of device it is. The kind comes from `src/identify.rs`, which reads
the manufacturer ID, Apple's message type, the services named and the declared
appearance, and only knows common values, such as `Apple Find My`, `Samsung` or
`Google Fast Pair`. Anything else stays unknown rather than guessed.
The list holds at most 256 devices, dropping the one heard from longest ago. The status
page shows the count, and `/bluetooth.json` the whole list. Nothing is written to
`/data` about the devices, and the log only records the scan starting and stopping.

Test results, from RAM boots:

- Turned on, the scan heard 10 to 11 devices in the first seconds and 31 to 41 within
  a minute. Most use random addresses, which change over time. As a passive scan, none
  sent a name.
- The active scan, after about 70 seconds, heard 49 devices: 9 with a name, such as
  `MJ_HT_V1`, two `LYWSD03MMC`, `Galaxy Watch7` and `32" Smart Monitor M7`, 32 with only
  a kind, mostly Apple devices, and 8 with neither. Whether the active scan costs Wi-Fi
  more than the passive one was not measured.
- After a restart with the choice kept, scanning started at 22.5 seconds, before Wi-Fi
  joined. `bluetooth off` stopped the scan, turned Bluetooth off in the driver and
  removed the setting.
- Reads of 4096 bytes made the driver log `count > BT_BUFFER_SIZE` for each read, so the
  init reads 1024 bytes at a time. The warning then stopped.

**The scan slows Wi-Fi.** Measured with `scripts/bench.ps1 -NetOnly`, alternating
on and off, 3 runs each, results in `benchmarks/amazing-a32/bluetooth-*.txt`:

| Scan | Wi-Fi receive with scan on, Mbit/s | Same with Bluetooth off |
|---|---|---|
| 30 ms of every 300 ms, round 1 | 35.1 to 36.0 | 42.1 to 44.3 |
| 30 ms of every 300 ms, round 2 | 26.0 to 32.1 | 39.6 to 40.9 |
| 30 ms of every 1.28 s, round 1 | 25.8 to 30.7 | 41.4 to 43.3 |
| 30 ms of every 1.28 s, round 2 | 28.9 to 34.0 | 41.8 to 43.4 |

Receiving was 18 to 32% slower with the scan on, and listening less often did not help,
so the 300 ms interval, which hears devices more often, was kept. Sending varied too
much to compare. The two chips share one radio and antenna, which would explain this.
Whether Bluetooth costs Wi-Fi when on but not scanning was tried once, and Wi-Fi varied
too much that evening to tell.

The HCI handling is in `src/bluetooth.rs`, with unit tests built from replies read on
this phone.

### BLE sensors

While the scan runs, temperature, humidity and battery are read from sensors that
broadcast them without a key, in `src/sensors.rs`:

| Format | Service data | Used by |
|---|---|---|
| Xiaomi MiBeacon, unencrypted | `0xFE95` | Older Xiaomi sensors, such as the round LYWSDCGQ |
| BTHome v2, unencrypted | `0xFCD2` | Shelly BLU and sensors with open firmware |
| pvvx and ATC1441 | `0x181A` | Xiaomi LYWSD03MMC and similar with custom firmware |

Encrypted MiBeacon, which newer Xiaomi sensors such as the LYWSD03MMC send with stock
firmware, needs each sensor's key and is skipped. A MiBeacon advertisement carries
either the climate or the battery, so a sensor's values are merged as they arrive.

Such a sensor is still recognized as encrypted, from the MiBeacon or BTHome header. The
panel's ADD page lists it marked `encrypted`, and a tap says it cannot be read instead
of saving it, and `bluetooth add` refuses it, because it would never show values. A
LYWSD03MMC next to the phone, product `0x055B`, sent frame control `0x5858`: MiBeacon
version 5, encrypted, with an object. It showed on the ADD page as `LYWSD03MMC 4a:70:05
encrypted`, a tap saved nothing, and `bluetooth add` refused it. Reading it would need
its key from the Xiaomi account, or the pvvx firmware on the sensor, which sends
unencrypted BTHome or pvvx frames that this already reads.

A neighbor's sensor is heard as well as one's own, so only saved sensors are shown and
recorded. The list is kept in `/data/a32/bluetooth-sensors`, at most 16. On the panel's
Bluetooth page, ADD lists the sensors heard but not saved, and a tap saves one. Tapping
a saved sensor asks before removing it, with the button below every sensor row, so a
double tap cannot remove one. On the console:

```text
bluetooth add 58:2d:34:36:74:93
bluetooth remove 58:2d:34:36:74:93
```

Saved sensors show under `bluetooth` on the console, on the panel, and in a Sensors table
on the status page, or `not heard` when out of range. `/bluetooth.json` still lists every
device, with a `sensor` object where one was read. Every 5 minutes the latest values of
each saved sensor heard since the last write are appended to `/data/a32/sensors.csv`,
which moves to `sensors.csv.1` at 1 MiB.

Tested from RAM boots, with `ui tap`: ADD listed the LYWSDCGQ with its values, a tap
saved it, and tapping it then YES, REMOVE removed it, each logged. On the console, adding
it twice, removing an address not saved and adding a bad address each gave the right
reply. The panel was also used by hand during these tests, which turned the scan off
and removed the sensor, as the log shows.

Test results, from a RAM boot: a survey of nearby advertisements
(`scripts/bluetooth/ble-kinds.py`) found service data `0xFE95` from one device, and
`mibeacon-kinds.py`
showed product `0x01AA`, the LYWSDCGQ, unencrypted, at -83 dBm. The scan then read it
as 26.00 C and 72.00%, then 25.90 C and 72.10%, and the history file got its header and
a row after 5 minutes. No battery value arrived in about 6 minutes, because this
sensor sends it less often. Whether the values match the sensor's display was not
checked. BTHome, pvvx and ATC1441 are only unit tested, from their published layouts,
because no such sensor was in range.

To repeat the first test, with `python3` installed in Alpine. Only the first half of each address is
printed, to keep nearby devices private:

```powershell
scripts/bt-test.ps1
```

## Power off and charging

With USB attached, a powered-off phone starts by itself in charger mode. The USB
console does not appear in this mode, and unplugging USB turns the phone off. The
init therefore restarts into normal mode when `boot_mode` is `8`. It stays in place
for `9`, the low battery charger mode.

This was tested with `poweroff` while USB was attached. The phone started twice,
then ended in mode `0` with the console ready about 60 seconds later. Each start
briefly showed USB ID `0E8D:2000`, which is MediaTek's preloader. With this system in
the boot partition, 3 of 3 tries returned to mode `0` in about 60 seconds with Wi-Fi
`ready`. With the saved network in place, 3 more tries each rejoined it and set the
clock about 29 seconds after the kernel started. A start without USB, by the power key,
also rejoined and set the clock at about 29 seconds, and the phone answered pings from
the PC. Plugging USB back in restored the console, and it charged. That is one replug,
so it does not rule out the charging problem under Known risks.

As a result the phone cannot stay off with USB attached. `poweroff` only stays off
without USB.

To leave charger mode if the console is missing, unplug USB, which turns the phone
off, press the power key with USB unplugged, then plug USB back in.

## Cameras

Rear GC5025 (2592x1944) and front GC2385 (1600x1200) work without Android's camera
stack: `camera rear|front NEW_FILE` on the console, photos at `/camera/rear.jpg`
and `/camera/front.jpg` and live video at `/camera/rear.mjpg` and
`/camera/front.mjpg` on the status page, and a CAMERA page on the panel. Photos are
half the sensor size (each 2x2 Bayer quad becomes one pixel), turned to the stock
camera's portrait orientation, with auto exposure, gray-world white balance and a
baseline JPEG encoder (`src/photo.rs`).

The web cameras need a password, set on the console with `web password NEW` (10 to
64 printable characters, no spaces; `web password off` removes it). It is kept in
`/data/a32/web-password`. Without one the camera addresses answer 403. Browsers
ask for it (HTTP Basic, any user name); a wrong one is logged and answered after a
second. The status page itself still needs no login. Plain HTTP sends the password
readably, so it only keeps out people on the network who do not watch its traffic.

Live video is MJPEG (`multipart/x-mixed-replace`), which browsers show directly. The
camera thread copies each finished frame out of the ION buffer before the ISP
overwrites it, develops it at a smaller size (rear 486x648 from every other quad,
front 600x800), and hands it to an encoding thread; a frame that arrives while
the encoder is busy is dropped, so the picture stays current. Developing and
encoding each split their rows between the four cores, and the JPEG has a restart
marker after every row of macroblocks so the rows can be encoded separately. A view
ends when the viewer leaves or after 10 minutes. Only one photo or view at a time:
anything else gets 503 "the camera is busy" at once.

Measured on the phone over Wi-Fi on 2026-11-07, quality 70: front 13.3 fps (73 KB
frames), rear 14.4 fps (67 KB) for 60 s, with the CPU going from 38 to 47 C. Before
the speed work it was 3.2 and 3.7 fps: the naive DCT, the per-pixel color
conversion and the float arithmetic per quad took the time, not the network
(sending is 1-4 ms per frame). Rear photos went from 1.7 s to 1.4 s. This image
then booted from flash, kept the password and streamed the front camera at 10 fps
within 30 s of starting.

The panel's CAMERA page has REAR, FRONT, LIVE and BACK. LIVE shows the rear camera's
live video on the screen; during it REAR and FRONT switch cameras, and the button
reads STOP. The camera thread hands each frame to a fitting thread when that one is
free, which shrinks it to 480x640 and leaves it in a one-frame slot that the panel
empties when it draws, so a slow step skips frames instead of falling behind. The view stops
on STOP, BACK, when the screen turns off, or after 3 minutes; frames keep the screen
on until then. It shares the one camera with the web, so a view running on either
makes the other fail as busy.

Tested from a RAM boot with `ui tap`, screenshots and the log: the rear and front
views, switching between them, BACK stopping the view, and the 3 minute limit, after
which the screen, still on, kept the last frame and said the view had ended. The
camera delivered 14.4-14.8 fps (rear) and 12.8 fps (front), and the panel first drew
8.1-8.3 and 6.9 fps of them. The log then showed, per frame, 74 and 96 ms fitting the
frame to 480x640, 30 ms drawing the page and 15 ms copying it to the screen. Fitting
now works out each column's source pixels once and sums whole rows, with output
equal to the old code's (a unit test compares them), and drawing an image goes row
by row: fitting took 28 and 49 ms, and the panel drew 13.8 fps of 16.0 (rear) and
10.6 of 12.7 (front). Fitting then moved to its own thread, and drawing an image
copies bytes when the screen's channels are whole bytes, as here (a unit test
compares it with encoding each pixel); the panel then took 3-4 ms for the status,
19-21 ms to draw and 13-14 ms to copy, and drew 15.6 fps of 15.7 (rear) and 11.6-13.0
of 11.8-13.6 (front): the camera, not the panel, now sets the rate. The first build never
drew a frame, because the panel loop handled every queued event before drawing and
live frames kept arriving; it now handles only what was queued when it woke.
After flashing, a rear view drew 7.9 fps for 9 s and STOP ended it; with the faster
fitting, a flashed rear view drew 13.7 fps of 14.2; with fitting in its own thread, a
flashed front view drew 14.0 fps of 14.8.

How it works (`src/camera.rs`), all verified on the phone on 2026-11-07:

- Sensor: kd_camera_hw set driver, then `0xC0046946` (set current sensor), without
  which open silently does nothing; open, preview scenario, shutter (feature 3004)
  and gain (3006, 64 = 1x).
- Seninf and MIPI: the stock HAL's NCSI2 setup (`libcam.halsensor.so`), rear on
  seninf 1/CSI0 with two lanes, front on seninf 2/CSI1 with one lane, mux 0 to TG1.
- ISP pass 1: `ISP_RESET`, then CAM register values read from the hardware while
  stock Android previewed (`src/camera_regs.rs`), IMGO into an ION buffer (heap
  0x400, M4U port 15), statistics modules off (with them on the pass stalls after
  about 20 lines). Frames are RAW10 packed least significant bit first, R first.

The register values came from a RAM-booted stock image with a patched ramdisk:
every SELinux domain permissive (`scripts/camera/sepolicy-permissive.py`) and a
root helper (`scripts/camera/camroot.sh`). Never flash such an image. The stock
camera then needed `setprop debug.lsc_mgr.enable 0` and `/data/nvram/media`,
because this phone's `/data` was wiped. Experiments are in `scripts/camera/`.

Tested from a RAM boot: rear photo 1.6-1.8 s, front 0.8 s; photos 47-690 KB. The same
image then booted from flash, joined Wi-Fi and served a front photo in about 1 s.
A live frame of a person in a room shows the front orientation is upright. The user
saw the front image mirrored on the panel, so the front camera now also flips it left
to right (`Rotation::Clockwise270Mirrored`); the other rotations are unchanged. After
flashing, the user confirmed the front image is no longer mirrored.
