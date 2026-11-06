//! PID 1 duties: filesystems, device nodes, logging, USB console, status screen
//! and child reaping.
//!
//! The stock 3.18 kernel has no devtmpfs, so `/dev` is a tmpfs populated by hand.
//! PID 1 must never exit, because that panics the kernel and reboots the phone.
//!
//! The phone can only reach fastboot from software, because no key combination
//! does it. So if this process panics early or the console cannot start, it reboots
//! to fastboot, where the system can be repaired. A panic after the system has run
//! for a while restarts it normally instead, and so does a service that stops
//! making progress, as decided in `watchdog`. A kernel panic restarts the phone by
//! itself, because the kernel is set to restart 1 second after one.

use crate::alpine;
use crate::dhcp::{self, MessageType};
use crate::display::Canvas;
use crate::fb::Framebuffer;
use crate::input::{self, Action, PowerKey, Touch, EVENT_LEN};
use crate::shell::{self, Platform};
use crate::sntp;
use crate::thermal::{self, Action as ThermalAction};
use crate::ui::{Request, Snapshot, Ui};
use crate::usb;
use crate::watchdog::{self, NetworkWatch, Service};
use crate::wifi;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Write};
use std::net::{Ipv4Addr, ToSocketAddrs, UdpSocket};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::raw::{c_char, c_int, c_long, c_short, c_uint, c_ulong, c_void};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, RwLock, TryLockError};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}

const POLLIN: c_short = 1;

extern "C" {
    fn poll(fds: *mut PollFd, count: c_ulong, timeout_ms: c_int) -> c_int;
    fn setsockopt(
        socket: c_int,
        level: c_int,
        name: c_int,
        value: *const c_void,
        length: c_uint,
    ) -> c_int;
    fn umount(target: *const c_char) -> c_int;
    fn statvfs(path: *const c_char, buffer: *mut c_void) -> c_int;
    fn mount(
        source: *const c_char,
        target: *const c_char,
        fstype: *const c_char,
        flags: c_ulong,
        data: *const c_void,
    ) -> c_int;
    fn mknod(path: *const c_char, mode: c_uint, dev: u64) -> c_int;
    fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
    fn klogctl(kind: c_int, buffer: *mut c_char, length: c_int) -> c_int;
    fn sync();
    fn syscall(number: c_long, ...) -> c_long;
}

const WNOHANG: c_int = 1;
const S_IFCHR: c_uint = 0o020000;
const S_IFBLK: c_uint = 0o060000;
const SERIAL_DEVICE: &str = "/dev/ttyGS0";

// Partitions exposed as /dev/NAME so that the console can back them up. Only
// `userdata` is written to, through the filesystem mounted on /data. `nvdata` holds
// calibration that cannot be restored from firmware, and is only mounted read-only.
const SYS_BLOCK: &str = "/sys/class/block";
const EXPOSED_PARTITIONS: &[&str] = &["boot", "recovery", "system", "userdata", "nvdata"];
const SERIAL_WAIT_SECONDS: u32 = 30;
// Must exceed SERIAL_WAIT_SECONDS, so that this only catches a stuck setup.
const CONSOLE_DEADLINE: Duration = Duration::from_secs(60);
const SYS_INPUT: &str = "/sys/class/input";
// The kernel's keypad device, which reports the power and volume keys.
const KEYPAD_NAME: &str = "mtk-kpd";
const KEYPAD_DEVICE: &str = "/dev/keypad";
// An exact name, because `mtk-tpd-kpd` is a separate device for the touch keys.
const TOUCH_NAME: &str = "mtk-tpd";
const TOUCH_DEVICE: &str = "/dev/touch";
const TOUCH_WAIT_SECONDS: u32 = 30;
// A copy of this process's log. The kernel log soon overwrites old lines,
// because the USB driver logs every transfer.
const INIT_LOG: &str = "/run/init.log";
const STOCK_SHELL: &str = "/system/bin/sh";
const STOCK_SHELL_OUTPUT: &str = "/run/stock-shell-output";
const STOCK_PATH: &str = "/system/bin:/system/xbin:/system/vendor/bin";
const PROPERTY_AREA: &str = "/dev/__properties__";
// `wmt_loader` cannot set this one itself without a property service, and
// `wmt_launcher` appears to wait for it.
const STOCK_PROPERTIES: &[(&str, &str)] = &[("service.wcn.driver.ready", "yes")];
const STOCK_IP: &str = "/system/bin/ip";
// The state of the Wi-Fi startup. Network details are never written here.
const WIFI_STATE: &str = "/run/wifi-state";
const WIFI_READY: &str = "ready";
const WIFI_INTERFACE: &str = "wlan0";
const FIRMWARE_PATH_PARAMETER: &str = "/sys/module/firmware_class/parameters/path";
const FIRMWARE_DIR: &str = "/system/vendor/firmware";
const SYS_CHAR_DEVICES: &str = "/sys/dev/char";
// Created when the kernel lists them. `wmt_loader` makes the kernel add some.
const WMT_DEVICES: &[&str] = &[
    "wmtdetect", "stpwmt", "wmtWifi", "stpbt", "stpgps", "stpant", "ttyMT2", "btif", "BTIF",
];
const WMT_WIFI_DEVICE: &str = "/dev/wmtWifi";
const WMT_LOADER: &str = "/system/vendor/bin/wmt_loader";
const WMT_LOADER_LOG: &str = "/run/wmt_loader.log";
const WMT_LAUNCHER: &str = "/system/vendor/bin/wmt_launcher";
const WMT_LAUNCHER_LOG: &str = "/run/wmt_launcher.log";
const WMT_LOADER_WAIT: Duration = Duration::from_secs(10);
// Time for `wmt_launcher` to load the firmware before Wi-Fi is turned on.
const WMT_LAUNCHER_DELAY: Duration = Duration::from_secs(8);
const WIFI_POWER_ATTEMPTS: u32 = 3;
const WIFI_INTERFACE_WAIT: Duration = Duration::from_secs(10);
const WPA_SUPPLICANT: &str = "/system/bin/wpa_supplicant";
const WPA_SUPPLICANT_LOG: &str = "/run/wpa.out";
const WPA_CLI: &str = "/system/bin/wpa_cli";
const WPA_CONFIG: &str = "/run/wpa.conf";
const WPA_CONTROL_DIR: &str = "/run/wpa";
// Android's `wpa_supplicant` runs as this user and keeps sockets in this directory.
const WIFI_UID: u32 = 1010;
const WIFI_DIRS: &[&str] = &["/data/misc/wifi", "/data/misc/wifi/sockets", WPA_CONTROL_DIR];
const WPA_PING_WAIT: Duration = Duration::from_secs(10);
// Time for a scan to finish before a network is selected.
const WIFI_SCAN_DELAY: Duration = Duration::from_secs(6);
const WIFI_JOIN_WAIT: Duration = Duration::from_secs(30);
const WPA_COMPLETED: &str = "COMPLETED";
const POLL_INTERVAL: Duration = Duration::from_secs(1);
const DATA_DEVICE: &str = "/dev/userdata";
const DATA_DIR: &str = "/data";
const E2FSCK: &str = "/system/bin/e2fsck";
const E2FSCK_LOG: &str = "/run/e2fsck.log";
const MS_NOATIME: c_ulong = 1024;
pub const MS_BIND: c_ulong = 4096;
pub const MS_REC: c_ulong = 16384;
// Settings kept across restarts. The directory is private, because the saved
// network holds a passphrase.
const SETTINGS_DIR: &str = "/data/a32";
const SAVED_NETWORK: &str = "/data/a32/wifi-network";
// The Wi-Fi driver reads its MAC address and calibration from these files. The stock
// system restores them from the `nvdata` partition, and without them the driver
// makes up an address on each start.
const NVDATA_DEVICE: &str = "/dev/nvdata";
const NVDATA_MOUNT: &str = "/run/nvdata";
const WIFI_CALIBRATION_SOURCE: &str = "/run/nvdata/APCFG/APRDEB";
const WIFI_CALIBRATION_TARGET: &str = "/data/nvram/APCFG/APRDEB";
const WIFI_CALIBRATION_FILES: &[&str] = &["WIFI", "WIFI_CUSTOM"];
const RESOLV_CONF: &str = "/run/resolv.conf";
// Without this the governor counts time waiting for the eMMC as idle and slows the CPU,
// which halved random reads and writes. It was measured to cost no idle temperature.
const CPU_IO_IS_BUSY: &str = "/sys/devices/system/cpu/cpufreq/interactive/io_is_busy";
const NETWORK_CHECK: Duration = Duration::from_secs(5);
// Reads `up` only when `wpa_supplicant` has finished connecting, and costs no process.
const WIFI_OPERSTATE: &str = "/sys/class/net/wlan0/operstate";
const LEASE_RETRY: Duration = Duration::from_secs(30);
// Names first, and an address as a fallback if the DNS server fails.
const TIME_SERVERS: &[&str] = &["time.google.com:123", "pool.ntp.org:123", "216.239.35.0:123"];
const TIME_REPLY_WAIT: Duration = Duration::from_secs(5);
const TIME_SYNC_INTERVAL: Duration = Duration::from_secs(6 * 3600);
const TIME_SYNC_RETRY: Duration = Duration::from_secs(60);
const STOCK_HWCLOCK: &str = "/system/bin/hwclock";
const SYS_RTC: &str = "/sys/class/rtc/rtc0/dev";
const RTC_DEVICE: &str = "/dev/rtc0";
const SYS_SETTIMEOFDAY: c_long = 79;
const THERMAL_ZONES: &str = "/sys/class/thermal";
const THERMAL_STATE: &str = "/run/thermal-state";
const THERMAL_CHECK: Duration = Duration::from_secs(10);
const CPU_ZONE: &str = "mtktscpu";
const BATTERY_ZONE: &str = "mtktsbattery";
const CPU_MAX_FREQUENCY: &str = "/sys/devices/system/cpu/cpu0/cpufreq/cpuinfo_max_freq";
const CPU_FREQUENCY_LIMIT: &str = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq";
// One of the speeds this CPU lists, a little over half of the top speed.
const CPU_CAPPED_KHZ: &str = "637000";
const CPU_CAP_DEGREES: i64 = 80;
// Replaces the limit in an image built to test the protection. Never flash one.
const CPU_CAP_TEST_DEGREES: Option<&str> = option_env!("A32_CPU_CAP_C");
const SOL_SOCKET: c_int = 1;
const SO_BINDTODEVICE: c_int = 25;
const DHCP_CLIENT_PORT: u16 = 68;
const DHCP_SERVER_PORT: u16 = 67;
const DHCP_ATTEMPTS: u32 = 4;
const DHCP_REPLY_WAIT: Duration = Duration::from_secs(3);
const MS_RDONLY: c_ulong = 1;
// `noload` skips the ext4 journal, because replaying it would write to the
// partition even for a read-only mount.
const SYSTEM_MOUNT_OPTIONS: &str = "noload";
const BOOT_MODE: &str = "/sys/class/BOOT/BOOT/boot/boot_mode";
// MediaTek's KERNEL_POWER_OFF_CHARGING: USB was plugged into a powered-off phone.
// The console never appears in this mode, so it would leave no way to fastboot.
// The low battery variant, mode 9, is left alone to avoid restarting on an empty
// battery.
const CHARGER_BOOT_MODE: &str = "8";
const LOW_BATTERY_BOOT_MODE: &str = "9";
const CONSOLE_READY_INFO: &str = "/run/console-ready";
const USB_STATE: &str = "/sys/class/android_usb/android0/state";
// Time for the kernel log and the screen to settle before restarting.
const FAIL_SAFE_DELAY: Duration = Duration::from_secs(3);
// The log kept across restarts, so that the reason for one can be read afterwards.
// Lines are appended without waiting for the eMMC, and ext4 commits them within
// about 5 seconds. One older file is kept, so this uses at most twice the limit.
const PERSISTENT_LOG_DIR: &str = "/data/a32/log";
const PERSISTENT_LOG: &str = "/data/a32/log/init.log";
const PERSISTENT_LOG_OLD: &str = "/data/a32/log/init.log.1";
const PERSISTENT_LOG_LIMIT: u64 = 1024 * 1024;
const WATCHDOG_CHECK: Duration = Duration::from_secs(10);
// How long a restart waits for the eMMC to take cached writes. A stuck eMMC must not
// stop the restart that may repair it.
const RESTART_SYNC_WAIT: Duration = Duration::from_secs(15);
const SYSRQ_TRIGGER: &str = "/proc/sysrq-trigger";
// Restarts in a row because the network was lost. The wait grows with each one.
const NETWORK_RESTARTS: &str = "/data/a32/network-restarts";
// How long the system must have run before a panic restarts it normally.
const STABLE_UPTIME: Duration = Duration::from_secs(600);
// Shorter limits for an image built to test the watchdog. Never flash one.
const STABLE_UPTIME_TEST: Option<&str> = option_env!("A32_STABLE_S");
const NETWORK_LIMIT_TEST: Option<&str> = option_env!("A32_NETWORK_LIMIT_S");
const AED_REBOOT_REASON: &str = "/proc/aed/reboot-reason";

/// When this process started. Watchdog times count from here, because the wall
/// clock jumps when it is set.
static STARTED: OnceLock<Instant> = OnceLock::new();

/// Each service's last heartbeat, as whole seconds since `STARTED` plus one, by
/// `Service::index`. Zero means not watched.
static HEARTBEATS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];

/// Held while writing a log line, and true once lines also go to the persistent log.
/// Holding it while switching on the persistent log keeps lines from going missing.
static PERSISTENT_LOG_ON: Mutex<bool> = Mutex::new(false);

/// Set when the console device exists. A host does not need to be attached, so
/// running on battery without USB does not count as a failure.
static CONSOLE_READY: AtomicBool = AtomicBool::new(false);

/// Whether the backlight is on and the panel is being drawn.
static SCREEN_ON: AtomicBool = AtomicBool::new(true);

/// What the panel reacts to.
enum PanelInput {
    Touch(Option<(i32, i32)>),
    Networks(Vec<wifi::Network>),
    Message(String),
}

/// Where `ui tap` sends its touches, set once the panel is running.
static PANEL_INPUT: OnceLock<mpsc::Sender<PanelInput>> = OnceLock::new();

/// Set when /data is a mounted filesystem, not a directory in memory.
static DATA_READY: AtomicBool = AtomicBool::new(false);

/// The address in use and when to renew it. Holding this also stops two DHCP
/// exchanges from competing for the client port.
struct LeaseState {
    applied: Option<(Ipv4Addr, u32)>,
    renew_at: Option<Instant>,
}

/// Held for reading while a child is being waited for, so that `reap_forever` does
/// not collect it first and hide how it ended.
static WAITED_CHILDREN: RwLock<()> = RwLock::new(());

static LEASE: Mutex<LeaseState> = Mutex::new(LeaseState { applied: None, renew_at: None });

/// A panic elsewhere must not leave networking locked for the rest of the session.
fn lease_state() -> MutexGuard<'static, LeaseState> {
    LEASE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A fault to inject for testing the fail-safe, chosen at build time with the
/// `A32_FAULT` environment variable: `panic`, `usb` or `hang`.
const FAULT: Option<&str> = option_env!("A32_FAULT");

const FB_SYSFS_DEV: &str = "/sys/class/graphics/fb0/dev";
const FB_DEVICE: &str = "/dev/fb0";
const DISPLAY_INFO: &str = "/run/display-info";
const BACKLIGHT: &str = "/sys/class/leds/lcd-backlight/brightness";
// A low level, because the screen stays on for the whole session.
const BACKLIGHT_LEVEL: &str = "64";
// Drawing the whole screen each second took much of the idle CPU time, so the screen
// turns off when nobody is using it.
const SCREEN_TIMEOUT: Duration = Duration::from_secs(60);
const BATTERY_CAPACITY: &str = "/sys/class/power_supply/battery/capacity";
const BATTERY_STATUS: &str = "/sys/class/power_supply/battery/status";

const SYSLOG_ACTION_READ_ALL: c_int = 3;
const SYSLOG_ACTION_CONSOLE_LEVEL: c_int = 8;
const SYSLOG_ACTION_SIZE_BUFFER: c_int = 10;
// Only emergencies reach the kernel console. The USB serial driver logs every
// transfer, and printing those to the console slows the transfers down.
const CONSOLE_LOG_LEVEL: c_int = 1;

// The reboot syscall number on 32-bit ARM EABI, the only supported target.
const SYS_REBOOT: c_long = 88;
const REBOOT_MAGIC1: c_ulong = 0xfee1_dead;
const REBOOT_MAGIC2: c_ulong = 0x2812_1969;
const REBOOT_CMD_RESTART: c_ulong = 0x0123_4567;
const REBOOT_CMD_RESTART2: c_ulong = 0xa1b2_c3d4;
const REBOOT_CMD_POWER_OFF: c_ulong = 0x4321_fedc;

/// Filesystems as (fstype, mount point). `/dev` must come first for logging.
const MOUNTS: &[(&str, &str)] = &[
    ("tmpfs", "/dev"),
    ("devpts", "/dev/pts"),
    ("tmpfs", "/dev/shm"),
    ("proc", "/proc"),
    ("sysfs", "/sys"),
    ("tmpfs", "/run"),
];

/// Character devices as (path, major, minor, permissions).
const DEVICES: &[(&str, u32, u32, c_uint)] = &[
    ("/dev/null", 1, 3, 0o666),
    ("/dev/kmsg", 1, 11, 0o600),
    ("/dev/console", 5, 1, 0o600),
    ("/dev/tty", 5, 0, 0o666),
    ("/dev/ptmx", 5, 2, 0o666),
    ("/dev/zero", 1, 5, 0o666),
    ("/dev/random", 1, 8, 0o666),
    ("/dev/urandom", 1, 9, 0o666),
];

/// Boots userspace. Never returns.
pub fn run() -> ! {
    STARTED.get_or_init(Instant::now);
    // The hook never returns, which stops panic=abort from killing PID 1.
    std::panic::set_hook(Box::new(|info| {
        let stable = STABLE_UPTIME_TEST
            .and_then(|text| text.parse().ok())
            .map_or(STABLE_UPTIME, Duration::from_secs);
        if watchdog::panic_restarts_normally(since_start(), stable) {
            restart(&format!("panic: {info}"))
        } else {
            fail_safe(&format!("panic: {info}"))
        }
    }));

    // Failures are collected because `/dev/kmsg` does not exist until the
    // device nodes below have been created.
    let mut errors = Vec::new();
    for &(fstype, target) in MOUNTS {
        if let Err(err) = mount_fs(fstype, target) {
            errors.push(format!("mount {target} failed: {err}"));
        }
    }
    for &(path, major, minor, perm) in DEVICES {
        if let Err(err) = make_char_device(path, major, minor, perm) {
            errors.push(format!("mknod {path} failed: {err}"));
        }
    }

    // SAFETY: this klogctl action does not use the buffer argument.
    if unsafe { klogctl(SYSLOG_ACTION_CONSOLE_LEVEL, ptr::null_mut(), CONSOLE_LOG_LEVEL) } < 0 {
        errors.push(format!("console log level failed: {}", io::Error::last_os_error()));
    }
    if let Err(err) = make_partition_devices() {
        errors.push(format!("partition devices failed: {err}"));
    }
    if let Err(err) = mount_system() {
        errors.push(format!("mount /system failed: {err}"));
    }
    if let Err(err) = prepare_etc() {
        errors.push(format!("/etc failed: {err}"));
    }
    if let Err(err) = write_property_area() {
        errors.push(format!("property area failed: {err}"));
    }

    for error in &errors {
        log(error);
    }
    log(&crate::probe::report());
    match fs::read_to_string(AED_REBOOT_REASON) {
        Ok(text) => log(&format!("previous reset: {}", watchdog::previous_reset(&text).unwrap_or("unknown"))),
        Err(err) => log(&format!("previous reset: {err}")),
    }

    // A software restart is a normal boot, even with USB attached.
    if fs::read_to_string(BOOT_MODE).is_ok_and(|mode| mode.trim() == CHARGER_BOOT_MODE) {
        log("charger boot: restarting into normal mode");
        let err = KernelPlatform.reboot(None);
        log(&format!("charger boot restart failed: {err}"));
    }

    if let Err(err) = thread::Builder::new().spawn(serial_console) {
        fail_safe(&format!("console thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(console_deadline) {
        log(&format!("console deadline thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(status_screen) {
        log(&format!("display thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(power_key) {
        log(&format!("power key thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(alpine::ssh_service) {
        log(&format!("ssh thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(thermal_service) {
        log(&format!("thermal thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(storage_and_network) {
        log(&format!("network thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(watchdog_service) {
        log(&format!("watchdog thread failed: {err}"));
    }

    log("boot complete");
    match FAULT {
        Some("panic") => panic!("injected fault"),
        Some("late-panic") => {
            let _ = thread::Builder::new().spawn(|| {
                thread::sleep(Duration::from_secs(90));
                panic!("injected late fault");
            });
        }
        _ => {}
    }
    reap_forever()
}

fn since_start() -> Duration {
    STARTED.get_or_init(Instant::now).elapsed()
}

/// Tells the watchdog that a service's loop went round.
fn heartbeat(service: Service) {
    HEARTBEATS[service.index()].store(since_start().as_secs() + 1, Ordering::Release);
}

/// Stops watching a service that ended on purpose, such as a missing display.
fn retire(service: Service) {
    HEARTBEATS[service.index()].store(0, Ordering::Release);
}

/// Restarts the phone when a service stops or the network stays lost. This must not
/// wait on anything a stuck service may hold, so the lease is only tried.
fn watchdog_service() {
    let first_limit = NETWORK_LIMIT_TEST
        .and_then(|text| text.parse().ok())
        .map_or(watchdog::NETWORK_LOST_LIMIT, Duration::from_secs);
    let mut network = NetworkWatch::default();
    // Read once /data is mounted. Before that no network can be saved.
    let mut network_restarts: Option<u32> = None;
    loop {
        thread::sleep(WATCHDOG_CHECK);
        let now = since_start();
        let beats = std::array::from_fn(|index| match HEARTBEATS[index].load(Ordering::Acquire) {
            0 => None,
            seconds => Some(Duration::from_secs(seconds - 1)),
        });
        if let Some((service, silent)) = watchdog::stalled(&beats, now) {
            restart(&format!("{} made no progress for {} s", service.name(), silent.as_secs()));
        }

        if !DATA_READY.load(Ordering::Acquire) {
            continue;
        }
        let restarts = *network_restarts.get_or_insert_with(|| {
            fs::read_to_string(NETWORK_RESTARTS).ok().and_then(|text| text.trim().parse().ok()).unwrap_or(0)
        });
        // Low battery mode leaves Wi-Fi off on purpose.
        let wifi_off = fs::read_to_string(WIFI_STATE).is_ok_and(|state| state.starts_with("off"));
        let saved = !wifi_off && Path::new(SAVED_NETWORK).exists();
        let Ok(lease) = LEASE.try_lock() else {
            continue;
        };
        let working = lease.applied.is_some()
            && fs::read_to_string(WIFI_OPERSTATE).is_ok_and(|state| state.trim() == "up");
        drop(lease);
        if working && restarts > 0 {
            match fs::remove_file(NETWORK_RESTARTS) {
                Ok(()) => log("watchdog: the network works again"),
                Err(err) => log(&format!("watchdog: restart count not cleared: {err}")),
            }
            network_restarts = Some(0);
        }
        let limit = watchdog::network_lost_limit(first_limit, restarts);
        if network.update(now, saved, working, limit) {
            let count = format!("{}\n", restarts + 1);
            let finished = within(RESTART_SYNC_WAIT, move || {
                if let Err(err) = write_private_file(NETWORK_RESTARTS, &count) {
                    log(&format!("watchdog: restart count not saved: {err}"));
                }
            });
            if !finished {
                log_without_waiting("watchdog: saving the restart count did not finish");
            }
            restart(&format!(
                "the saved network did not work for {} s, restart {} in a row for this",
                limit.as_secs(),
                restarts + 1
            ));
        }
    }
}

/// Restarts normally, for a fault a restart may repair. Never returns.
fn restart(reason: &str) -> ! {
    log_without_waiting(&format!("watchdog: restarting: {reason}"));
    let synced = within(RESTART_SYNC_WAIT, || {
        // SAFETY: `sync` takes no arguments and has no memory safety requirements.
        unsafe { sync() }
    });
    if !synced {
        log_without_waiting("watchdog: cached writes did not finish, restarting anyway");
    }
    // SAFETY: this reboot command takes no pointer arguments.
    unsafe { syscall(SYS_REBOOT, REBOOT_MAGIC1, REBOOT_MAGIC2, REBOOT_CMD_RESTART, 0) };
    log_without_waiting(&format!("watchdog: restart failed: {}, trying sysrq", io::Error::last_os_error()));
    if let Err(err) = fs::write(SYSRQ_TRIGGER, "b") {
        log_without_waiting(&format!("watchdog: sysrq failed: {err}"));
    }
    park()
}

/// Runs `work` on its own thread and waits for it at most `limit`, for work that a
/// stuck eMMC can hold up. Returns whether it finished. Unfinished work goes on in
/// the background.
fn within(limit: Duration, work: impl FnOnce() + Send + 'static) -> bool {
    let (done, finished) = mpsc::channel();
    let started = thread::Builder::new().spawn(move || {
        work();
        let _ = done.send(());
    });
    started.is_ok() && finished.recv_timeout(limit).is_ok()
}

/// Reboots to fastboot, the only way to repair the system. Never returns.
fn fail_safe(reason: &str) -> ! {
    log(&format!("fail-safe: {reason}"));
    thread::sleep(FAIL_SAFE_DELAY);
    let err = KernelPlatform.reboot(Some("bootloader"));
    log(&format!("fail-safe reboot failed: {err}"));
    park()
}

/// Catches a console setup that hangs instead of failing.
fn console_deadline() {
    thread::sleep(CONSOLE_DEADLINE);
    if !CONSOLE_READY.load(Ordering::Acquire) {
        fail_safe("console setup did not finish");
    }
}

/// Runs the status screen. A display failure leaves the console usable.
fn status_screen() {
    let result = run_status_screen();
    retire(Service::Panel);
    if let Err(err) = result {
        log(&format!("display stopped: {err}"));
        let _ = fs::write(DISPLAY_INFO, format!("stopped: {err}\n"));
    }
}

fn run_status_screen() -> io::Result<()> {
    let dev = fs::read_to_string(FB_SYSFS_DEV)?;
    let (major, minor) = usb::parse_dev(&dev)
        .ok_or_else(|| io::Error::other(format!("bad device number: {dev}")))?;
    make_char_device(FB_DEVICE, major, minor, 0o600)?;

    let mut framebuffer = Framebuffer::open(FB_DEVICE)?;
    let format = framebuffer.format();
    let unblank = framebuffer.unblank();
    let backlight = fs::write(BACKLIGHT, BACKLIGHT_LEVEL);
    let info = format!("{format:?}\nunblank: {unblank:?}\nbacklight: {backlight:?}\n");
    log(&info);
    let _ = fs::write(DISPLAY_INFO, info);

    // `input` stays alive here, so the receiver below never sees a disconnect even if
    // the touch thread stops.
    let (input, inputs) = mpsc::channel();
    let sender = input.clone();
    if let Err(err) = thread::Builder::new().spawn(move || touch_screen(sender)) {
        log(&format!("touch thread failed: {err}"));
    }
    let _ = PANEL_INPUT.set(input.clone());

    // Slow work runs apart from the panel, so that the screen keeps responding.
    let (requests, pending) = mpsc::channel();
    let replies = input.clone();
    if let Err(err) = thread::Builder::new().spawn(move || panel_requests(pending, replies)) {
        log(&format!("panel thread failed: {err}"));
    }

    let mut canvas = Canvas::new(format);
    let mut panel = Ui::default();
    let mut last_use = Instant::now();
    let mut was_on = true;
    // A touch that wakes the screen must not also press a button under the finger.
    let mut waking = false;
    loop {
        heartbeat(Service::Panel);
        let on = SCREEN_ON.load(Ordering::Acquire);
        if on && !was_on {
            // Turned on by the power key.
            last_use = Instant::now();
        }
        if !on && was_on {
            panel.reset();
        }
        was_on = on;
        if on && last_use.elapsed() >= SCREEN_TIMEOUT {
            set_screen(false);
            panel.reset();
            was_on = false;
        } else if on {
            panel.render(&mut canvas, &snapshot());
            framebuffer.present(canvas.pixels())?;
        }

        // Redraw when something changes, and otherwise once a second.
        if let Ok(first) = inputs.recv_timeout(Duration::from_secs(1)) {
            for event in std::iter::once(first).chain(inputs.try_iter()) {
                match event {
                    PanelInput::Touch(position) => {
                        last_use = Instant::now();
                        if !SCREEN_ON.load(Ordering::Acquire) {
                            set_screen(true);
                            was_on = true;
                            waking = true;
                        }
                        if waking {
                            waking = position.is_some();
                            continue;
                        }
                        if let Some(request) = panel.touch(position) {
                            let _ = requests.send(request);
                        }
                    }
                    PanelInput::Networks(networks) => panel.set_networks(networks),
                    PanelInput::Message(message) => {
                        last_use = Instant::now();
                        panel.set_message(message);
                    }
                }
            }
        }
    }
}

/// Reads the values the home page shows. None of this starts a process, because it
/// runs once a second while the screen is on.
fn snapshot() -> Snapshot {
    let read = |path: &str| fs::read_to_string(path).unwrap_or_default();
    let degrees = |zone| {
        zone_temperature(zone).map_or("?".to_string(), |value| format!("{:.1} C", value as f64 / 1000.0))
    };
    let uptime: f64 = read("/proc/uptime")
        .split_whitespace()
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0.0);
    let seconds = uptime as u64;
    let memory = read("/proc/meminfo");
    let megabytes = |name: &str| {
        memory
            .lines()
            .find_map(|line| line.strip_prefix(name)?.split_whitespace().next()?.parse::<u64>().ok())
            .map_or(0, |kib| kib / 1024)
    };
    let connected = read(WIFI_OPERSTATE).trim() == "up";
    let applied = lease_state().applied;
    let level = wifi::wireless_level(&read("/proc/net/wireless"), WIFI_INTERFACE);
    let network = load_saved_network()
        .map(|saved| wifi::display_ssid(&saved.ssid_bytes()))
        .unwrap_or_else(|| "not saved".to_string());
    Snapshot {
        address: match applied {
            Some((address, _)) if connected => address.to_string(),
            _ => "none".to_string(),
        },
        network: if connected { network } else { format!("off: {}", read(WIFI_STATE).trim()) },
        signal: match level {
            Some(level) if connected => format!("{level} dBm"),
            _ => "-".to_string(),
        },
        ssh: if alpine::ssh_running() { "on, port 22".to_string() } else { "off".to_string() },
        uptime: format!("{}d {:02}:{:02}:{:02}", seconds / 86400, seconds / 3600 % 24, seconds / 60 % 60, seconds % 60),
        load: read("/proc/loadavg").split_whitespace().take(3).collect::<Vec<_>>().join(" "),
        cpu_temperature: degrees(CPU_ZONE),
        memory: format!("{} of {} MB free", megabytes("MemAvailable:"), megabytes("MemTotal:")),
        disk: free_disk_text(),
        battery: format!("{}% {}", read(BATTERY_CAPACITY).trim(), read(BATTERY_STATUS).trim()),
        battery_temperature: degrees(BATTERY_ZONE),
    }
}

/// Describes the free space on /data.
fn free_disk_text() -> String {
    // musl's 32-bit `struct statvfs`: `f_bsize` and `f_frsize` are 32 bits, then
    // `f_blocks`, `f_bfree` and `f_bavail` are 64 bits. This buffer holds all of it.
    let mut fields = [0u64; 16];
    let Ok(path) = cstring(DATA_DIR) else {
        return "?".to_string();
    };
    // SAFETY: `path` is a valid string, and `fields` is larger than the structure, and
    // aligned for it, for the call duration.
    if !DATA_READY.load(Ordering::Acquire) || unsafe { statvfs(path.as_ptr(), fields.as_mut_ptr().cast()) } != 0 {
        return "not mounted".to_string();
    }
    let block_size = fields[0] >> 32;
    let mebibytes = |blocks: u64| blocks * block_size / (1024 * 1024);
    format!("{} of {} MB free", mebibytes(fields[3]), mebibytes(fields[1]))
}

/// Does the panel's slow work, one request at a time.
fn panel_requests(requests: mpsc::Receiver<Request>, replies: mpsc::Sender<PanelInput>) {
    let say = |text: String| {
        let _ = replies.send(PanelInput::Message(text));
    };
    for request in requests {
        match request {
            Request::Restart => {
                log("panel: restarting");
                let err = KernelPlatform.reboot(None);
                say(format!("Restart failed: {err}"));
            }
            Request::PowerOff => {
                log("panel: powering off");
                let err = KernelPlatform.power_off();
                say(format!("Power off failed: {err}"));
            }
            Request::Scan => match scan_networks() {
                Ok(networks) => {
                    say(format!("{} networks found", networks.len()));
                    let _ = replies.send(PanelInput::Networks(networks));
                }
                Err(err) => say(format!("Scan failed: {err}")),
            },
            // The passphrase goes no further than `wifi_join`, which does not log it.
            Request::Join(credentials) => match KernelPlatform.wifi_join(&credentials) {
                Ok(_) => {
                    let address = lease_state().applied.map(|(address, _)| address.to_string());
                    say(format!("Joined: {}", address.unwrap_or_default()));
                }
                Err(err) => say(format!("Not joined: {err}")),
            },
            Request::Forget => match KernelPlatform.wifi_forget() {
                Ok(_) => say("Network forgotten".to_string()),
                Err(err) => say(format!("Forget failed: {err}")),
            },
        }
    }
}

fn scan_networks() -> io::Result<Vec<wifi::Network>> {
    if fs::read_to_string(WIFI_STATE)?.trim() != WIFI_READY {
        return Err(io::Error::other("Wi-Fi is not ready"));
    }
    // A scan can be refused while one is running, and the results are still useful.
    let _ = wpa_cli(&["scan"]);
    thread::sleep(WIFI_SCAN_DELAY);
    Ok(wifi::parse_scan_results(&wpa_cli(&["scan_results"])?))
}

fn set_screen(on: bool) {
    SCREEN_ON.store(on, Ordering::Release);
    let level = if on { BACKLIGHT_LEVEL } else { "0" };
    if let Err(err) = fs::write(BACKLIGHT, level) {
        log(&format!("backlight failed: {err}"));
    }
}

/// Times the parts of drawing one frame. The status screen may also be drawing if
/// it is on, so run this with the screen off.
fn display_bench() -> io::Result<String> {
    const FRAMES: u32 = 60;
    let mut framebuffer = Framebuffer::open(FB_DEVICE)?;
    let mut canvas = Canvas::new(framebuffer.format());
    let panel = Ui::default();
    let mut status = snapshot();
    let (mut render, mut write, mut pan) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    let (mut render_max, mut write_max, mut pan_max) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);

    panel.render(&mut canvas, &status);
    let first = Instant::now();
    let first_rows = framebuffer.copy_changes(canvas.pixels())?;
    let whole_frame = first.elapsed();
    framebuffer.pan()?;

    let start = Instant::now();
    for frame in 0..FRAMES {
        // Changes what a running system changes each second, only faster.
        status.uptime = format!("0d 00:00:{frame:02}");
        let t0 = Instant::now();
        panel.render(&mut canvas, &status);
        let t1 = Instant::now();
        framebuffer.copy_changes(canvas.pixels())?;
        let t2 = Instant::now();
        framebuffer.pan()?;
        let t3 = Instant::now();
        render += t1 - t0;
        write += t2 - t1;
        pan += t3 - t2;
        render_max = render_max.max(t1 - t0);
        write_max = write_max.max(t2 - t1);
        pan_max = pan_max.max(t3 - t2);
    }
    let total = start.elapsed();
    let average = |sum: Duration| sum.as_secs_f64() * 1000.0 / f64::from(FRAMES);
    let longest = |item: Duration| item.as_secs_f64() * 1000.0;
    Ok(format!(
        "display.whole_frame rows={first_rows} value={:.1} unit=ms\ndisplay.render value={:.1} max={:.1} unit=ms\ndisplay.write value={:.1} max={:.1} unit=ms\ndisplay.pan value={:.1} max={:.1} unit=ms\ndisplay.rate value={:.1} unit=frames/s\n",
        longest(whole_frame), average(render), longest(render_max), average(write), longest(write_max), average(pan), longest(pan_max),
        f64::from(FRAMES) / total.as_secs_f64(),
    ))
}

/// Sends touches to the panel. A failure leaves the rest running.
fn touch_screen(updates: mpsc::Sender<PanelInput>) {
    if let Err(err) = run_touch_screen(&updates) {
        log(&format!("touch stopped: {err}"));
    }
}

fn run_touch_screen(updates: &mpsc::Sender<PanelInput>) -> io::Result<()> {
    let (major, minor) = wait_for_touch_device()?;
    make_char_device(TOUCH_DEVICE, major, minor, 0o600)?;
    let mut device = File::open(TOUCH_DEVICE)?;
    let mut touch = Touch::default();
    loop {
        let mut raw = [0u8; EVENT_LEN];
        device.read_exact(&mut raw)?;
        if let Some(update) = touch.event(&raw) {
            updates.send(PanelInput::Touch(update)).map_err(io::Error::other)?;
        }
    }
}

/// Runs the power key. A failure leaves the console and the screen usable.
fn power_key() {
    if let Err(err) = run_power_key() {
        log(&format!("power key stopped: {err}"));
    }
}

fn run_power_key() -> io::Result<()> {
    let (major, minor) = find_input_device(KEYPAD_NAME)?;
    make_char_device(KEYPAD_DEVICE, major, minor, 0o600)?;
    let mut device = File::open(KEYPAD_DEVICE)?;

    let start = Instant::now();
    let mut key = PowerKey::default();
    loop {
        let timeout = key.wait_limit(start.elapsed());
        let action = if wait_readable(&device, timeout)? {
            let mut raw = [0u8; EVENT_LEN];
            device.read_exact(&mut raw)?;
            input::parse_event(&raw).and_then(|event| key.event(event, start.elapsed()))
        } else {
            key.tick(start.elapsed())
        };
        match action {
            Some(Action::ToggleScreen) => set_screen(!SCREEN_ON.load(Ordering::Acquire)),
            Some(Action::PowerOff) => {
                log("power key: powering off");
                let err = KernelPlatform.power_off();
                log(&format!("power off failed: {err}"));
            }
            None => {}
        }
    }
}

/// Mounts /data, starts Wi-Fi, then keeps the network working. This runs apart from
/// the console, so that a slow or stuck step cannot take away the way to fastboot.
fn storage_and_network() {
    if let Err(err) = fs::write(CPU_IO_IS_BUSY, "1") {
        log(&format!("tuning: io_is_busy not set: {err}"));
    }
    // Before the network starts. SSH stays off if this fails.
    match alpine::apply_firewall() {
        Ok(()) => log("firewall: ready"),
        Err(err) => log(&format!("firewall: failed: {err}")),
    }
    match mount_data() {
        Ok(()) => {
            DATA_READY.store(true, Ordering::Release);
            log("data: mounted");
            if let Err(err) = start_persistent_log() {
                log(&format!("log: not kept across restarts: {err}"));
            }
            if let Err(err) = restore_wifi_calibration() {
                log(&format!("wifi: no calibration from nvdata: {err}"));
            }
        }
        // Never formatted here: that would erase /data after any passing fault.
        Err(err) => log(&format!("data: not mounted, nothing will be saved: {err}")),
    }

    if fs::read_to_string(BOOT_MODE).is_ok_and(|mode| mode.trim() == LOW_BATTERY_BOOT_MODE) {
        set_wifi_state("off: low battery");
        return;
    }
    set_wifi_state("starting");
    let start = Instant::now();
    if let Err(err) = start_wifi() {
        set_wifi_state(&format!("failed: {err}"));
        return;
    }
    log(&format!("wifi: started in {} s", start.elapsed().as_secs()));
    set_wifi_state(WIFI_READY);
    keep_network_working();
}

fn mount_data() -> io::Result<()> {
    let check = run_child(logged_command(E2FSCK, E2FSCK_LOG)?.args(["-p", DATA_DEVICE]))?;
    // Codes 1 and 2 mean it fixed something. Others are left to the mount below.
    if !check.success() {
        log(&format!("data: e2fsck ended with {check}, see {E2FSCK_LOG}"));
    }
    fs::create_dir_all(DATA_DIR)?;
    let source = cstring(DATA_DEVICE)?;
    let target = cstring(DATA_DIR)?;
    let fstype = cstring("ext4")?;
    // SAFETY: all pointers are valid NUL-terminated strings for the call duration.
    let rc = unsafe {
        mount(source.as_ptr(), target.as_ptr(), fstype.as_ptr(), MS_NOATIME, ptr::null())
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    fs::create_dir_all(SETTINGS_DIR)?;
    fs::set_permissions(SETTINGS_DIR, fs::Permissions::from_mode(0o700))
}

/// Copies the Wi-Fi calibration out of `nvdata` the first time. A failure only means
/// the driver makes up a MAC address and uses its default calibration.
fn restore_wifi_calibration() -> io::Result<()> {
    let target = Path::new(WIFI_CALIBRATION_TARGET);
    if WIFI_CALIBRATION_FILES.iter().all(|name| target.join(name).exists()) {
        return Ok(());
    }
    fs::create_dir_all(target)?;
    fs::create_dir_all(NVDATA_MOUNT)?;
    let source = cstring(NVDATA_DEVICE)?;
    let mount_point = cstring(NVDATA_MOUNT)?;
    let fstype = cstring("ext4")?;
    let options = cstring(SYSTEM_MOUNT_OPTIONS)?;
    // SAFETY: all pointers are valid NUL-terminated strings for the call duration.
    let rc = unsafe {
        mount(
            source.as_ptr(),
            mount_point.as_ptr(),
            fstype.as_ptr(),
            MS_RDONLY,
            options.as_ptr().cast(),
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    let copied = WIFI_CALIBRATION_FILES.iter().try_for_each(|name| {
        fs::copy(Path::new(WIFI_CALIBRATION_SOURCE).join(name), target.join(name)).map(|_| ())
    });
    // SAFETY: `mount_point` is a valid NUL-terminated string for the call duration.
    if unsafe { umount(mount_point.as_ptr()) } != 0 {
        log(&format!("wifi: could not unmount nvdata: {}", io::Error::last_os_error()));
    }
    copied?;
    // SAFETY: `sync` takes no arguments and has no memory safety requirements.
    unsafe { sync() };
    log("wifi: copied the calibration from nvdata");
    Ok(())
}

/// Replaces a file in one step, so that a power cut leaves the old or the new one.
fn write_private_file(path: &str, contents: &str) -> io::Result<()> {
    let temporary = format!("{path}.new");
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(SETTINGS_DIR)?.sync_all()
}

fn load_saved_network() -> Option<wifi::Credentials> {
    if !DATA_READY.load(Ordering::Acquire) {
        return None;
    }
    let text = fs::read_to_string(SAVED_NETWORK).ok()?;
    let credentials = wifi::Credentials::from_saved(&text);
    if credentials.is_none() {
        log("wifi: the saved network is damaged");
    }
    credentials
}

/// Rejoins the saved network, renews the lease and keeps the clock set.
/// `wpa_supplicant` itself reconnects after a lost connection.
fn keep_network_working() -> ! {
    if let Some(credentials) = load_saved_network() {
        log("wifi: joining the saved network");
        // On a failure `wpa_supplicant` keeps the network and goes on trying.
        if let Err(err) = join_network(&credentials) {
            log(&format!("wifi: saved network not joined yet: {err}"));
        }
    }

    let mut connected = false;
    let mut next_time_sync = Instant::now();
    loop {
        heartbeat(Service::Network);
        let now_connected =
            fs::read_to_string(WIFI_OPERSTATE).is_ok_and(|state| state.trim() == "up");
        if now_connected != connected {
            log(if now_connected { "wifi: connected" } else { "wifi: connection lost" });
            connected = now_connected;
        }
        if !connected {
            // The next connection may be to a different network.
            lease_state().renew_at = None;
        } else {
            renew_lease_if_due();
            if lease_state().applied.is_some() && Instant::now() >= next_time_sync {
                next_time_sync = Instant::now()
                    + match sync_clock() {
                        Ok(()) => TIME_SYNC_INTERVAL,
                        Err(err) => {
                            log(&format!("time: {err}"));
                            TIME_SYNC_RETRY
                        }
                    };
            }
        }
        thread::sleep(NETWORK_CHECK);
    }
}

fn renew_lease_if_due() {
    let due = lease_state().renew_at.is_none_or(|time| Instant::now() >= time);
    if !due {
        return;
    }
    match renew_lease(WIFI_INTERFACE) {
        Ok(text) => log(&format!("dhcp: {text}")),
        Err(err) => {
            log(&format!("dhcp: {err}"));
            lease_state().renew_at = Some(Instant::now() + LEASE_RETRY);
        }
    }
}

/// Gets a lease and applies it. The address is left alone if it did not change,
/// because removing it would break open connections.
fn renew_lease(interface: &str) -> io::Result<String> {
    let mut state = lease_state();
    let lease = acquire_lease(interface)?;
    let netmask = lease.netmask.unwrap_or(Ipv4Addr::new(255, 255, 255, 0));
    let prefix = dhcp::prefix_length(netmask)
        .ok_or_else(|| io::Error::other(format!("bad netmask: {netmask}")))?;

    if state.applied != Some((lease.address, prefix)) {
        run_ip(&["addr", "flush", "dev", interface])?;
        run_ip(&["addr", "add", &format!("{}/{prefix}", lease.address), "dev", interface])?;
        state.applied = Some((lease.address, prefix));
    }
    if let Some(router) = lease.router {
        run_ip(&["route", "replace", "default", "via", &router.to_string(), "dev", interface])?;
    }
    if let Some(dns) = lease.dns {
        fs::write(RESOLV_CONF, format!("nameserver {dns}\n"))?;
    }
    let renew_seconds = wifi::renew_after(lease.lease_seconds);
    state.renew_at = Some(Instant::now() + Duration::from_secs(renew_seconds));
    Ok(format!(
        "address {}/{prefix} router {:?} dns {:?} lease {:?} s, renewing in {renew_seconds} s",
        lease.address, lease.router, lease.dns, lease.lease_seconds
    ))
}

/// Sets the clock from the first time server that answers, then saves it in the
/// RTC, so that the time is about right after a restart with no network.
fn sync_clock() -> io::Result<()> {
    let mut last_error = io::Error::other("no time server configured");
    for server in TIME_SERVERS {
        match ask_time_server(server) {
            Ok(unix) => {
                set_clock(unix)?;
                let saved = save_clock_to_rtc();
                log(&format!("time: set to {unix} from {server}, rtc saved: {saved:?}"));
                return Ok(());
            }
            Err(err) => last_error = io::Error::other(format!("{server}: {err}")),
        }
    }
    Err(last_error)
}

fn ask_time_server(server: &str) -> io::Result<u64> {
    let target = server
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::other("the name has no address"))?;
    let socket = UdpSocket::bind(("0.0.0.0", 0))?;
    socket.set_read_timeout(Some(TIME_REPLY_WAIT))?;
    socket.connect(target)?;
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0x6132_0003, |elapsed| elapsed.as_nanos() as u64);
    socket.send(&sntp::build_request(nonce))?;
    let mut reply = [0u8; sntp::MESSAGE_LEN];
    let length = socket.recv(&mut reply)?;
    sntp::parse_reply(&reply[..length], nonce)
        .ok_or_else(|| io::Error::other("unusable reply"))
}

fn set_clock(unix: u64) -> io::Result<()> {
    // This kernel's 32-bit `struct timeval`. `sntp::parse_reply` keeps `unix` in range.
    let time: [i32; 2] = [unix as i32, 0];
    // SAFETY: `time` is a valid timeval for the call duration, and the time zone
    // argument may be null.
    let rc = unsafe { syscall(SYS_SETTIMEOFDAY, time.as_ptr(), ptr::null::<c_void>()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn save_clock_to_rtc() -> io::Result<()> {
    if !Path::new(RTC_DEVICE).exists() {
        let dev = fs::read_to_string(SYS_RTC)?;
        let (major, minor) = usb::parse_dev(&dev)
            .ok_or_else(|| io::Error::other(format!("bad device number: {dev}")))?;
        make_char_device(RTC_DEVICE, major, minor, 0o600)?;
    }
    let status = run_child(
        stock_command(STOCK_HWCLOCK).args(["-w", "-u"]).stdout(Stdio::null()).stderr(Stdio::null()),
    )?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("hwclock ended with {status}")))
    }
}

/// Finds a thermal zone by its type and returns millidegrees.
fn zone_temperature(wanted: &str) -> Option<i64> {
    for entry in fs::read_dir(THERMAL_ZONES).ok()?.flatten() {
        let path = entry.path();
        if fs::read_to_string(path.join("type")).is_ok_and(|name| name.trim() == wanted) {
            return fs::read_to_string(path.join("temp")).ok()?.trim().parse().ok();
        }
    }
    None
}

/// Slows the CPU when it is too hot, and as a last resort turns the phone off.
fn thermal_service() {
    let cap_at = CPU_CAP_TEST_DEGREES.and_then(|text| text.parse().ok()).unwrap_or(CPU_CAP_DEGREES);
    let mut policy = thermal::Policy::new(cap_at);
    loop {
        heartbeat(Service::Thermal);
        let cpu = zone_temperature(CPU_ZONE);
        let battery = zone_temperature(BATTERY_ZONE);
        let result = match policy.decide(cpu, battery) {
            ThermalAction::None => Ok(()),
            ThermalAction::Cap => {
                log(&format!("thermal: capping the CPU, cpu {cpu:?} battery {battery:?}"));
                fs::write(CPU_FREQUENCY_LIMIT, CPU_CAPPED_KHZ)
            }
            ThermalAction::Release => {
                log(&format!("thermal: releasing the CPU, cpu {cpu:?} battery {battery:?}"));
                fs::read(CPU_MAX_FREQUENCY).and_then(|top| fs::write(CPU_FREQUENCY_LIMIT, top))
            }
            ThermalAction::PowerOff => {
                log(&format!("thermal: powering off, cpu {cpu:?} battery {battery:?}"));
                Err(KernelPlatform.power_off())
            }
        };
        if let Err(err) = result {
            log(&format!("thermal: action failed: {err}"));
        }
        let _ = fs::write(
            THERMAL_STATE,
            format!("capped: {}\ncpu: {cpu:?}\nbattery: {battery:?}\n", policy.is_capped()),
        );
        if FAULT == Some("stall") {
            log("thermal: injected stall");
            park();
        }
        thread::sleep(THERMAL_CHECK);
    }
}

fn set_wifi_state(state: &str) {
    log(&format!("wifi: {state}"));
    let _ = fs::write(WIFI_STATE, format!("{state}\n"));
}

/// Runs the steps the stock system runs, in the order that was tested by hand.
fn start_wifi() -> io::Result<()> {
    fs::write(FIRMWARE_PATH_PARAMETER, FIRMWARE_DIR)?;

    make_wmt_devices()?;
    let loader = run_child(&mut logged_command(WMT_LOADER, WMT_LOADER_LOG)?)?;
    if !loader.success() {
        log(&format!("wifi: wmt_loader ended with {loader}"));
    }
    let loaded = wait_until(WMT_LOADER_WAIT, || {
        make_wmt_devices().is_ok() && Path::new(WMT_WIFI_DEVICE).exists()
    });
    if !loaded {
        return Err(io::Error::other("wmt_loader did not add wmtWifi"));
    }

    logged_command(WMT_LAUNCHER, WMT_LAUNCHER_LOG)?
        .args(["-p", "/vendor/firmware/"])
        .spawn()?;
    thread::sleep(WMT_LAUNCHER_DELAY);
    power_on_wifi()?;
    run_ip(&["link", "set", WIFI_INTERFACE, "up"])?;

    prepare_wpa_directories()?;
    logged_command(WPA_SUPPLICANT, WPA_SUPPLICANT_LOG)?
        .args(["-i", WIFI_INTERFACE, "-Dnl80211"])
        .arg(format!("-c{WPA_CONFIG}"))
        .spawn()?;
    let answers = wait_until(WPA_PING_WAIT, || {
        wpa_cli(&["ping"]).is_ok_and(|reply| reply.trim() == "PONG")
    });
    if !answers {
        return Err(io::Error::other("wpa_supplicant did not answer"));
    }
    Ok(())
}

/// Creates the devices the kernel lists now and that do not exist yet.
fn make_wmt_devices() -> io::Result<()> {
    for entry in fs::read_dir(SYS_CHAR_DEVICES)? {
        let Ok(uevent) = fs::read_to_string(entry?.path().join("uevent")) else {
            continue;
        };
        let Some(device) = wifi::parse_uevent(&uevent) else {
            continue;
        };
        let path = format!("/dev/{}", device.name);
        if WMT_DEVICES.contains(&device.name.as_str()) && !Path::new(&path).exists() {
            make_char_device(&path, device.major, device.minor, 0o600)?;
        }
    }
    Ok(())
}

/// Asks for Wi-Fi power until the interface appears.
fn power_on_wifi() -> io::Result<()> {
    let interface = format!("/sys/class/net/{WIFI_INTERFACE}");
    for attempt in 1..=WIFI_POWER_ATTEMPTS {
        if let Err(err) = fs::write(WMT_WIFI_DEVICE, "1\n") {
            log(&format!("wifi: power on attempt {attempt} failed: {err}"));
        }
        if wait_until(WIFI_INTERFACE_WAIT, || Path::new(&interface).exists()) {
            log(&format!("wifi: {WIFI_INTERFACE} appeared after power on attempt {attempt}"));
            return Ok(());
        }
    }
    Err(io::Error::other(format!("{WIFI_INTERFACE} did not appear")))
}

fn prepare_wpa_directories() -> io::Result<()> {
    for directory in WIFI_DIRS {
        fs::create_dir_all(directory)?;
        std::os::unix::fs::chown(directory, Some(WIFI_UID), Some(WIFI_UID))?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o770))?;
    }
    for directory in ["/data", "/data/misc"] {
        fs::set_permissions(directory, fs::Permissions::from_mode(0o755))?;
    }
    fs::write(WPA_CONFIG, format!("ctrl_interface={WPA_CONTROL_DIR}\n"))?;
    fs::set_permissions(WPA_CONFIG, fs::Permissions::from_mode(0o644))
}

/// Polls a condition once a second. It is checked at least once.
fn wait_until(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    loop {
        if done() {
            return true;
        }
        if start.elapsed() >= limit {
            return false;
        }
        thread::sleep(POLL_INTERVAL);
    }
}

/// A stock program with the environment it expects.
pub fn stock_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command
        .env("PATH", STOCK_PATH)
        .env("ANDROID_ROOT", "/system")
        .env("ANDROID_DATA", "/data")
        .stdin(Stdio::null());
    command
}

/// A stock program that writes its output and errors to a new file.
fn logged_command(program: &str, log_path: &str) -> io::Result<Command> {
    let log = File::create(log_path)?;
    let mut command = stock_command(program);
    command.stdout(log.try_clone()?).stderr(log);
    Ok(command)
}

/// Runs a program to its end and returns how it ended.
pub fn run_child(command: &mut Command) -> io::Result<ExitStatus> {
    let _waiting = WAITED_CHILDREN.read().unwrap_or_else(|poisoned| poisoned.into_inner());
    command.status()
}

/// Runs `wpa_cli` directly, without a shell, so that a passphrase needs no
/// escaping. Its arguments are not logged for the same reason.
fn wpa_cli(arguments: &[&str]) -> io::Result<String> {
    let _waiting = WAITED_CHILDREN.read().unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut child = stock_command(WPA_CLI)
        .args(["-p", WPA_CONTROL_DIR, "-i", WIFI_INTERFACE])
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut output = Vec::new();
    if let Some(mut stdout) = child.stdout.take() {
        stdout.read_to_end(&mut output)?;
    }
    child.wait()?;
    Ok(String::from_utf8_lossy(&output).into_owned())
}

/// Runs a `wpa_cli` command that answers `OK`. Only its name reaches an error.
fn wpa_command(arguments: &[&str]) -> io::Result<()> {
    let reply = wpa_cli(arguments)?;
    if reply.trim() == "OK" {
        Ok(())
    } else {
        let name = arguments.first().copied().unwrap_or_default();
        Err(io::Error::other(format!("wpa_cli {name} answered: {}", reply.trim())))
    }
}

fn join_network(credentials: &wifi::Credentials) -> io::Result<()> {
    wpa_command(&["remove_network", "all"])?;
    wpa_command(&["scan"])?;
    thread::sleep(WIFI_SCAN_DELAY);

    let reply = wpa_cli(&["add_network"])?;
    let id = reply.trim();
    if id.parse::<u32>().is_err() {
        return Err(io::Error::other(format!("wpa_cli add_network answered: {id}")));
    }
    wpa_command(&["set_network", id, "ssid", &credentials.ssid])?;
    wpa_command(&["set_network", id, "psk", &credentials.psk])?;
    wpa_command(&["set_network", id, "key_mgmt", "WPA-PSK"])?;
    wpa_command(&["enable_network", id])?;
    wpa_command(&["select_network", id])?;

    let mut state = String::new();
    let joined = wait_until(WIFI_JOIN_WAIT, || {
        let status = wpa_cli(&["status"]).unwrap_or_default();
        state = wifi::wpa_state(&status).unwrap_or("unknown").to_string();
        state == WPA_COMPLETED
    });
    if joined {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!("did not join the network, last state {state}"),
        ))
    }
}

/// The touch driver can register after this process starts, so a missing device
/// is only an error once the wait is over.
fn wait_for_touch_device() -> io::Result<(u32, u32)> {
    for attempt in 0..TOUCH_WAIT_SECONDS {
        match find_input_device(TOUCH_NAME) {
            Ok(device) => {
                log(&format!("touch device found after {attempt} s"));
                return Ok(device);
            }
            Err(_) => thread::sleep(Duration::from_secs(1)),
        }
    }
    find_input_device(TOUCH_NAME)
}

fn find_input_device(wanted: &str) -> io::Result<(u32, u32)> {
    for entry in fs::read_dir(SYS_INPUT)? {
        let path = entry?.path();
        let matches = fs::read_to_string(path.join("device/name"))
            .is_ok_and(|name| name.trim() == wanted);
        // `inputN` entries also match, but only `eventN` entries have `dev`.
        if matches {
            if let Ok(dev) = fs::read_to_string(path.join("dev")) {
                return usb::parse_dev(&dev)
                    .ok_or_else(|| io::Error::other(format!("bad device number: {dev}")));
            }
        }
    }
    Err(io::Error::other(format!("{wanted} not found")))
}

/// Returns whether `file` has data before the timeout. `None` waits forever.
fn wait_readable(file: &File, timeout: Option<Duration>) -> io::Result<bool> {
    let mut entry = PollFd { fd: file.as_raw_fd(), events: POLLIN, revents: 0 };
    // Round up, so that a long press is not reported a moment too early and
    // then polled again with a timeout of zero.
    let timeout_ms = timeout.map_or(-1, |limit| limit.as_millis() as c_int + 1);
    // SAFETY: `entry` is one valid pollfd for the duration of the call.
    let ready = unsafe { poll(&mut entry, 1, timeout_ms) };
    if ready < 0 {
        let err = io::Error::last_os_error();
        return if err.kind() == io::ErrorKind::Interrupted { Ok(false) } else { Err(err) };
    }
    Ok(ready > 0)
}

/// Brings up the USB serial port and serves the shell on it forever.
fn serial_console() {
    match FAULT {
        Some("usb") => fail_safe("injected fault"),
        Some("hang") => park(),
        _ => {}
    }
    if let Err(err) = usb::enable_serial(Path::new(usb::ANDROID_USB)) {
        fail_safe(&format!("usb gadget failed: {err}"));
    }
    let Some((major, minor)) = wait_for_serial_device() else {
        fail_safe("usb serial device did not appear");
    };
    if let Err(err) = make_char_device(SERIAL_DEVICE, major, minor, 0o600) {
        fail_safe(&format!("mknod {SERIAL_DEVICE} failed: {err}"));
    }
    CONSOLE_READY.store(true, Ordering::Release);
    log("usb serial console ready");
    record_console_ready();

    // Open and read fail while no host is attached, and input ends on
    // disconnect, so keep retrying for the lifetime of the system.
    loop {
        if let Err(err) = serve_shell() {
            log(&format!("console session ended: {err}"));
        }
        thread::sleep(Duration::from_secs(1));
    }
}

/// Records when the console became ready and whether a USB host was attached.
/// The kernel log is too busy to keep this, because it soon overwrites old lines.
fn record_console_ready() {
    let read = |path| fs::read_to_string(path).unwrap_or_else(|err| format!("{err}\n"));
    let uptime = read("/proc/uptime");
    let state = read(USB_STATE);
    let _ = fs::write(CONSOLE_READY_INFO, format!("uptime: {uptime}usb state: {state}"));
}

fn wait_for_serial_device() -> Option<(u32, u32)> {
    for _ in 0..SERIAL_WAIT_SECONDS {
        if let Ok(text) = fs::read_to_string(usb::SERIAL_SYSFS_DEV) {
            return usb::parse_dev(&text);
        }
        thread::sleep(Duration::from_secs(1));
    }
    None
}

fn serve_shell() -> io::Result<()> {
    let port = OpenOptions::new().read(true).write(true).open(SERIAL_DEVICE)?;
    let reader = BufReader::new(File::try_clone(&port)?);
    shell::run(reader, port, &KernelPlatform)
}

struct KernelPlatform;

impl Platform for KernelPlatform {
    fn kernel_log(&self) -> io::Result<Vec<u8>> {
        // SAFETY: a null buffer with length 0 is valid for the size query.
        let size = unsafe { klogctl(SYSLOG_ACTION_SIZE_BUFFER, ptr::null_mut(), 0) };
        if size < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0u8; size as usize];
        // SAFETY: `buffer` is writable for the `size` bytes passed to the kernel.
        let read = unsafe { klogctl(SYSLOG_ACTION_READ_ALL, buffer.as_mut_ptr().cast(), size) };
        if read < 0 {
            return Err(io::Error::last_os_error());
        }
        buffer.truncate(read as usize);
        Ok(buffer)
    }

    fn reboot(&self, target: Option<&str>) -> io::Error {
        // SAFETY: `sync` takes no arguments and has no memory safety requirements.
        unsafe { sync() };
        let rc = match target.map(cstring) {
            None => {
                // SAFETY: this reboot command takes no pointer arguments.
                unsafe { syscall(SYS_REBOOT, REBOOT_MAGIC1, REBOOT_MAGIC2, REBOOT_CMD_RESTART, 0) }
            }
            Some(Ok(name)) => {
                // SAFETY: `name` is a valid NUL-terminated string for the call duration.
                unsafe {
                    syscall(
                        SYS_REBOOT,
                        REBOOT_MAGIC1,
                        REBOOT_MAGIC2,
                        REBOOT_CMD_RESTART2,
                        name.as_ptr(),
                    )
                }
            }
            Some(Err(err)) => return err,
        };
        debug_assert!(rc < 0);
        io::Error::last_os_error()
    }

    fn run_stock_shell(&self, line: &str) -> io::Result<Vec<u8>> {
        // Output goes to a file instead of a pipe, so that a background program
        // started by the line cannot keep this call waiting.
        let output = File::create(STOCK_SHELL_OUTPUT)?;
        run_child(
            stock_command(STOCK_SHELL)
                .args(["-c", line])
                .stdout(output.try_clone()?)
                .stderr(output),
        )?;
        fs::read(STOCK_SHELL_OUTPUT)
    }

    fn bench(&self, arguments: &[&str]) -> io::Result<String> {
        match arguments {
            ["display"] => display_bench(),
            ["net", port, ..] => {
                let port = port.parse().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "bad number")
                })?;
                alpine::with_open_port(port, || crate::bench::run(arguments))
            }
            _ => crate::bench::run(arguments),
        }
    }

    fn tap(&self, x: i32, y: i32) -> io::Result<()> {
        let panel = PANEL_INPUT.get().ok_or_else(|| io::Error::other("the panel is not running"))?;
        for position in [Some((x, y)), None] {
            panel.send(PanelInput::Touch(position)).map_err(io::Error::other)?;
        }
        Ok(())
    }

    fn run_alpine(&self, line: &str) -> io::Result<Vec<u8>> {
        alpine::run_line(line)
    }

    fn receive_file(&self, path: &str, port: u16) -> io::Result<u64> {
        alpine::with_open_port(port, || crate::transfer::receive(path, port))
    }

    fn wifi_status(&self) -> io::Result<String> {
        let state = fs::read_to_string(WIFI_STATE)?;
        let state = state.trim();
        if state != WIFI_READY {
            return Ok(format!("startup: {state}"));
        }
        Ok(format!("startup: {state}\n{}", wpa_cli(&["status"])?.trim_end()))
    }

    fn wifi_join(&self, credentials: &wifi::Credentials) -> io::Result<String> {
        let state = fs::read_to_string(WIFI_STATE)?;
        if state.trim() != WIFI_READY {
            return Err(io::Error::other(format!("Wi-Fi is not ready: {}", state.trim())));
        }
        let text = match join_network(credentials).and_then(|()| self.dhcp(WIFI_INTERFACE)) {
            Ok(text) => text,
            Err(err) => {
                // Also clears the passphrase from `wpa_supplicant`, and stops its retries.
                let _ = wpa_command(&["remove_network", "all"]);
                return Err(err);
            }
        };
        // Saved only now, so that a mistyped passphrase does not replace a good one.
        let saved = if DATA_READY.load(Ordering::Acquire) {
            write_private_file(SAVED_NETWORK, &credentials.to_saved())
                .map_or_else(|err| format!("not saved: {err}"), |()| "saved".to_string())
        } else {
            "not saved: /data is not mounted".to_string()
        };
        Ok(format!("{text}\n{saved}"))
    }

    fn wifi_forget(&self) -> io::Result<String> {
        match fs::remove_file(SAVED_NETWORK) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
            _ => {}
        }
        if DATA_READY.load(Ordering::Acquire) {
            File::open(SETTINGS_DIR)?.sync_all()?;
        }
        wpa_command(&["remove_network", "all"])?;
        let mut state = lease_state();
        state.renew_at = None;
        state.applied = None;
        run_ip(&["addr", "flush", "dev", WIFI_INTERFACE])?;
        Ok("left the network and erased its saved details".to_string())
    }

    fn dhcp(&self, interface: &str) -> io::Result<String> {
        renew_lease(interface)
    }

    fn power_off(&self) -> io::Error {
        // SAFETY: `sync` takes no arguments and has no memory safety requirements.
        unsafe { sync() };
        // SAFETY: this reboot command takes no pointer arguments.
        unsafe { syscall(SYS_REBOOT, REBOOT_MAGIC1, REBOOT_MAGIC2, REBOOT_CMD_POWER_OFF, 0) };
        io::Error::last_os_error()
    }
}

/// Runs the stock `ip` program with its output discarded.
fn run_ip(arguments: &[&str]) -> io::Result<()> {
    let status = run_child(
        Command::new(STOCK_IP)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("ip {arguments:?} failed: {status}")))
    }
}

/// Runs one DHCP exchange: Discover, Offer, Request, Ack.
fn acquire_lease(interface: &str) -> io::Result<dhcp::Reply> {
    let mac_text = fs::read_to_string(format!("/sys/class/net/{interface}/address"))?;
    let mac = dhcp::parse_mac(&mac_text)
        .ok_or_else(|| io::Error::other(format!("bad MAC address: {mac_text}")))?;

    let socket = UdpSocket::bind(("0.0.0.0", DHCP_CLIENT_PORT))?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(DHCP_REPLY_WAIT))?;
    bind_to_device(&socket, interface)?;

    let xid = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0x6132_0001, |elapsed| elapsed.subsec_nanos() ^ elapsed.as_secs() as u32);

    let offer = exchange(&socket, &dhcp::build_message(xid, mac, None), xid, mac, MessageType::Offer)?;
    let server = offer
        .server
        .ok_or_else(|| io::Error::other("offer has no server identifier"))?;
    let request = dhcp::build_message(xid, mac, Some((offer.address, server)));
    exchange(&socket, &request, xid, mac, MessageType::Ack)
}

/// Sends a message until the wanted reply arrives or the attempts run out.
fn exchange(
    socket: &UdpSocket,
    message: &[u8],
    xid: u32,
    mac: [u8; 6],
    wanted: MessageType,
) -> io::Result<dhcp::Reply> {
    let mut buffer = [0u8; 1500];
    for _ in 0..DHCP_ATTEMPTS {
        socket.send_to(message, ("255.255.255.255", DHCP_SERVER_PORT))?;
        // A timeout ends this attempt. Unrelated packets keep it waiting for at
        // most one more timeout each, which is fine for a manual command.
        while let Ok((length, _)) = socket.recv_from(&mut buffer) {
            match dhcp::parse_reply(&buffer[..length], xid, mac) {
                Some(reply) if reply.kind == wanted => return Ok(reply),
                Some(reply) if reply.kind == MessageType::Nak => {
                    return Err(io::Error::other("server refused the request"));
                }
                _ => {}
            }
        }
    }
    Err(io::Error::new(io::ErrorKind::TimedOut, format!("no DHCP {wanted:?} received")))
}

fn bind_to_device(socket: &UdpSocket, interface: &str) -> io::Result<()> {
    // SAFETY: the pointer and length describe `interface` for the call duration,
    // and the kernel only reads from it.
    let rc = unsafe {
        setsockopt(
            socket.as_raw_fd(),
            SOL_SOCKET,
            SO_BINDTODEVICE,
            interface.as_ptr().cast(),
            interface.len() as c_uint,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub fn cstring(value: &str) -> io::Result<CString> {
    CString::new(value).map_err(|_| io::ErrorKind::InvalidInput.into())
}

fn mount_fs(fstype: &str, target: &str) -> io::Result<()> {
    fs::create_dir_all(target)?;
    let source = cstring(fstype)?;
    let target = cstring(target)?;
    // SAFETY: all pointers are valid NUL-terminated strings for the call duration.
    let rc = unsafe { mount(source.as_ptr(), target.as_ptr(), source.as_ptr(), 0, ptr::null()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Writes the file bionic reads properties from. Bionic rejects it unless root
/// owns it and only root can write to it.
fn write_property_area() -> io::Result<()> {
    let bytes = crate::properties::build_area(STOCK_PROPERTIES).map_err(io::Error::other)?;
    fs::write(PROPERTY_AREA, bytes)?;
    fs::set_permissions(PROPERTY_AREA, fs::Permissions::from_mode(0o444))
}

/// Makes a writable /etc with links into /system/etc, which stock programs read,
/// and a link to the DNS settings, which change at run time.
fn prepare_etc() -> io::Result<()> {
    fs::create_dir_all("/etc")?;
    for entry in fs::read_dir("/system/etc")? {
        let entry = entry?;
        let name = entry.file_name();
        if name != "resolv.conf" {
            std::os::unix::fs::symlink(entry.path(), Path::new("/etc").join(name))?;
        }
    }
    std::os::unix::fs::symlink(RESOLV_CONF, "/etc/resolv.conf")
}

/// Mounts the stock system partition read-only, for its Wi-Fi programs and
/// firmware. The rest of this system does not depend on it.
fn mount_system() -> io::Result<()> {
    fs::create_dir_all("/system")?;
    let source = cstring("/dev/system")?;
    let target = cstring("/system")?;
    let fstype = cstring("ext4")?;
    let options = cstring(SYSTEM_MOUNT_OPTIONS)?;
    // SAFETY: all pointers are valid NUL-terminated strings for the call duration.
    let rc = unsafe {
        mount(
            source.as_ptr(),
            target.as_ptr(),
            fstype.as_ptr(),
            MS_RDONLY,
            options.as_ptr().cast(),
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    std::os::unix::fs::symlink("/system/vendor", "/vendor")
}

/// Encodes a device number the same way as musl's `makedev`.
fn makedev(major: u32, minor: u32) -> u64 {
    let (major, minor) = (u64::from(major), u64::from(minor));
    ((major & 0xfffff000) << 32)
        | ((major & 0x00000fff) << 8)
        | ((minor & 0xffffff00) << 12)
        | (minor & 0x000000ff)
}

fn make_partition_devices() -> io::Result<()> {
    for entry in fs::read_dir(SYS_BLOCK)? {
        let path = entry?.path();
        let Ok(uevent) = fs::read_to_string(path.join("uevent")) else {
            continue;
        };
        let Some(name) = crate::dump::partition_name(&uevent) else {
            continue;
        };
        if EXPOSED_PARTITIONS.contains(&name) {
            let dev = fs::read_to_string(path.join("dev"))?;
            let (major, minor) = usb::parse_dev(&dev)
                .ok_or_else(|| io::Error::other(format!("bad device number: {dev}")))?;
            make_device(&format!("/dev/{name}"), S_IFBLK | 0o400, major, minor)?;
        }
    }
    Ok(())
}

fn make_char_device(path: &str, major: u32, minor: u32, perm: c_uint) -> io::Result<()> {
    make_device(path, S_IFCHR | perm, major, minor)
}

fn make_device(path: &str, mode: c_uint, major: u32, minor: u32) -> io::Result<()> {
    let path = cstring(path)?;
    // SAFETY: `path` is a valid NUL-terminated string for the call duration.
    let rc = unsafe { mknod(path.as_ptr(), mode, makedev(major, minor)) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Writes one record per line to the kernel log. Errors are ignored because
/// there is nowhere else to report them this early in boot.
pub fn log(message: &str) {
    write_log(message, true);
}

/// Logs without waiting for another thread's log line, for a restart that must
/// happen even if a stuck eMMC holds that thread. The line may then miss /data.
fn log_without_waiting(message: &str) {
    write_log(message, false);
}

fn write_log(message: &str, wait: bool) {
    if let Ok(mut kmsg) = OpenOptions::new().write(true).open("/dev/kmsg") {
        for line in message.lines() {
            let _ = writeln!(kmsg, "a32-init: {line}");
        }
    }
    let guard = if wait {
        Some(PERSISTENT_LOG_ON.lock().unwrap_or_else(|poisoned| poisoned.into_inner()))
    } else {
        match PERSISTENT_LOG_ON.try_lock() {
            Ok(guard) => Some(guard),
            Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    };
    let persistent = guard.as_deref().copied().unwrap_or(false);
    let uptime = fs::read_to_string("/proc/uptime").unwrap_or_default();
    let seconds = uptime.split_whitespace().next().unwrap_or("?");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(INIT_LOG) {
        for line in message.lines() {
            let _ = writeln!(file, "[{seconds}] {line}");
        }
    }
    if persistent {
        if let Ok(mut file) = open_persistent_log() {
            let now = wall_clock_text();
            for line in message.lines() {
                let _ = writeln!(file, "{now} [{seconds}] {line}");
            }
        }
    }
}

/// Starts copying log lines to /data, beginning with the lines of this session so far.
fn start_persistent_log() -> io::Result<()> {
    let mut on = PERSISTENT_LOG_ON.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    fs::create_dir_all(PERSISTENT_LOG_DIR)?;
    fs::set_permissions(PERSISTENT_LOG_DIR, fs::Permissions::from_mode(0o700))?;
    let earlier = fs::read_to_string(INIT_LOG).unwrap_or_default();
    let mut file = open_persistent_log()?;
    // Lines from before /data was mounted get the time they were copied, but keep
    // their uptime. The clock comes from the RTC until the network sets it.
    let now = wall_clock_text();
    writeln!(file, "{now} --- session start ---")?;
    for line in earlier.lines() {
        writeln!(file, "{now} {line}")?;
    }
    *on = true;
    Ok(())
}

/// Opens the persistent log for appending, first moving a full one aside.
fn open_persistent_log() -> io::Result<File> {
    if fs::metadata(PERSISTENT_LOG).is_ok_and(|meta| meta.len() >= PERSISTENT_LOG_LIMIT) {
        fs::rename(PERSISTENT_LOG, PERSISTENT_LOG_OLD)?;
    }
    OpenOptions::new().create(true).append(true).mode(0o600).open(PERSISTENT_LOG)
}

fn wall_clock_text() -> String {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or_else(|_| "before-1970".to_string(), |elapsed| watchdog::utc_text(elapsed.as_secs()))
}

/// Collects children nobody is waiting for, such as orphans. It skips a round while
/// a child is being waited for, without queueing, so that it never delays others.
fn reap_forever() -> ! {
    loop {
        heartbeat(Service::Reaper);
        if let Ok(_nobody_waiting) = WAITED_CHILDREN.try_write() {
            loop {
                let mut status = 0;
                // SAFETY: `status` is a valid writable c_int for the call duration.
                if unsafe { waitpid(-1, &mut status, WNOHANG) } <= 0 {
                    break;
                }
            }
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn park() -> ! {
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
