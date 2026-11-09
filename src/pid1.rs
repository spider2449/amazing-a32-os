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
use crate::bluetooth;
use crate::sensors;
use crate::charging::{self, Event as PowerEvent};
use crate::dhcp::{self, MessageType};
use crate::desktop;
use crate::display::{Canvas, Format};
use crate::camera;
use crate::fb::Framebuffer;
use crate::photo;
use crate::input::{self, Action, Gesture, PowerKey, Touch, EVENT_LEN};
use crate::pointer::Pointer;
use crate::shell::{self, Platform};
use crate::settings::Settings;
use crate::sntp;
use crate::thermal::{self, Action as ThermalAction};
use crate::ui::{self, Request, SensorRow, Snapshot, Ui};
use crate::usb;
use crate::watchdog::{self, NetworkWatch, Service};
use crate::web::{self, Route};
use crate::wifi;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Write};
use std::io::{Seek, SeekFrom};
use std::net::{Ipv4Addr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::raw::{c_char, c_int, c_long, c_short, c_uint, c_ulong, c_void};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock, TryLockError};
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
    fn sethostname(name: *const c_char, length: usize) -> c_int;
    fn statvfs(path: *const c_char, buffer: *mut c_void) -> c_int;
    fn mount(
        source: *const c_char,
        target: *const c_char,
        fstype: *const c_char,
        flags: c_ulong,
        data: *const c_void,
    ) -> c_int;
    fn mknod(path: *const c_char, mode: c_uint, dev: u64) -> c_int;
    fn swapon(path: *const c_char, flags: c_int) -> c_int;
    fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
    fn klogctl(kind: c_int, buffer: *mut c_char, length: c_int) -> c_int;
    fn sync();
    fn syscall(number: c_long, ...) -> c_long;
    fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int;
    fn sendto(
        socket: c_int,
        buffer: *const c_void,
        length: usize,
        flags: c_int,
        address: *const c_void,
        address_length: c_uint,
    ) -> isize;
    fn close(fd: c_int) -> c_int;
    fn if_nametoindex(name: *const c_char) -> c_uint;
}

/// `struct sockaddr_ll` from `<linux/if_packet.h>`, for sending raw frames.
#[repr(C)]
struct SockaddrLl {
    family: u16,
    protocol: u16,
    interface_index: c_int,
    hardware_type: u16,
    packet_type: u8,
    address_length: u8,
    address: [u8; 8],
}

const AF_PACKET: c_int = 17;
const SOCK_RAW: c_int = 3;
const ARP_HARDWARE_ETHER: u16 = 1;
// Sent a few times, because one broadcast frame over Wi-Fi can be lost.
const ARP_ANNOUNCE_GAPS: [Duration; 3] =
    [Duration::ZERO, Duration::from_secs(1), Duration::from_secs(3)];

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
const ZRAM_SYS: &str = "/sys/block/zram0";
const ZRAM_DEVICE: &str = "/dev/zram0";
const MEMINFO: &str = "/proc/meminfo";
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
const WIFI_LEAVE_WAIT: Duration = Duration::from_secs(3);
// Time for the deauthentication frame and its retries before the radio scans.
const WIFI_DEAUTH_SETTLE: Duration = Duration::from_secs(1);
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
// A slow or silent client is dropped after this, because requests are served one at
// a time.
const WEB_TIMEOUT: Duration = Duration::from_secs(5);
const WEB_LOG_LINES: usize = 40;
// Enough for the lines above, so that a 1 MiB log is not read for each request.
const WEB_LOG_BYTES: u64 = 16 * 1024;
// MediaTek's raw Bluetooth device. Opening it powers Bluetooth on, closing it off.
const BLUETOOTH_DEVICE: &str = "/dev/stpbt";
// Holds `on` while the scan is wanted. Off unless chosen, because Bluetooth shares
// the radio with Wi-Fi.
const BLUETOOTH_SETTING: &str = "/data/a32/bluetooth";
const USB_NETWORK_SETTING: &str = "/data/a32/usb-network";
// Long enough for the console to send its reply before the USB link drops.
const USB_SWITCH_DELAY: Duration = Duration::from_secs(1);
const USB_CONSOLE_WAIT: Duration = Duration::from_secs(60);
const USB_CHECK: Duration = Duration::from_millis(200);
const USB_INTERFACE_WAIT: Duration = Duration::from_secs(5);
const USB_LEASE_WAIT: Duration = Duration::from_secs(1);
const USB_LEASE_SECONDS: u32 = 86400;
const BLUETOOTH_REPLY_WAIT: Duration = Duration::from_secs(2);
const BLUETOOTH_RETRY: Duration = Duration::from_secs(30);
const BLUETOOTH_IDLE_CHECK: Duration = Duration::from_secs(2);
const BLUETOOTH_LIST_LIMIT: usize = 20;
// As many as the panel's Bluetooth page has rows for.
const BLUETOOTH_PANEL_NEARBY: usize = 6;
// The latest values of each BLE sensor, appended this often while scanning.
const SENSOR_HISTORY: &str = "/data/a32/sensors.csv";
const SENSOR_HISTORY_OLD: &str = "/data/a32/sensors.csv.1";
const SENSOR_HISTORY_LIMIT: u64 = 1024 * 1024;
const SENSOR_HISTORY_INTERVAL: Duration = Duration::from_secs(300);
// The saved sensors, one address per line.
const SAVED_SENSORS: &str = "/data/a32/bluetooth-sensors";
/// The password for the cameras on the status page; without it they are off.
const WEB_PASSWORD: &str = "/data/a32/web-password";
/// After a wrong camera password the web service waits this long, which slows
/// guessing, since it serves one request at a time.
const WRONG_PASSWORD_DELAY: Duration = Duration::from_secs(1);
/// Live video ends after this long; reloading the page starts it again.
const LIVE_LIMIT: Duration = Duration::from_secs(10 * 60);
const LIVE_QUALITY: u32 = 70;

/// The BLE scan: whether it is wanted, what it is doing, and what it heard.
struct BluetoothState {
    wanted: bool,
    state: String,
    devices: bluetooth::Devices,
    /// The sensors chosen to show and record.
    saved: sensors::Saved,
}

static BLUETOOTH: Mutex<BluetoothState> = Mutex::new(BluetoothState {
    wanted: false,
    state: String::new(),
    devices: bluetooth::Devices::new(),
    saved: sensors::Saved::new(),
});

fn bluetooth_state() -> MutexGuard<'static, BluetoothState> {
    BLUETOOTH.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// When this process started. Watchdog times count from here, because the wall
/// clock jumps when it is set.
static STARTED: OnceLock<Instant> = OnceLock::new();

/// Each service's last heartbeat, as whole seconds since `STARTED` plus one, by
/// `Service::index`. Zero means not watched.
static HEARTBEATS: [AtomicU64; Service::COUNT] = [const { AtomicU64::new(0) }; Service::COUNT];

/// Held while writing a log line, and true once lines also go to the persistent log.
/// Holding it while switching on the persistent log keeps lines from going missing.
static PERSISTENT_LOG_ON: Mutex<bool> = Mutex::new(false);

/// Set when the console device exists. A host does not need to be attached, so
/// running on battery without USB does not count as a failure.
static CONSOLE_READY: AtomicBool = AtomicBool::new(false);
static USB_NETWORK: Mutex<Option<UsbNetwork>> = Mutex::new(None);

/// The DHCP server for the USB network, while that network is on.
struct UsbNetwork {
    running: Arc<AtomicBool>,
    server: thread::JoinHandle<()>,
}

/// Whether the backlight is on and the panel is being drawn.
static SCREEN_ON: AtomicBool = AtomicBool::new(true);

/// What the panel reacts to.
enum PanelInput {
    Touch(Option<(i32, i32)>),
    Networks(Vec<wifi::Network>),
    Message(String),
    /// A photo for the camera page.
    Photo(photo::Rgb),
    /// What the console's shell wrote, and that it ended.
    TerminalOutput(Vec<u8>),
    TerminalEnded,
    /// The live view with this number has a new frame waiting, or ended and why.
    LiveFrame(u32),
    LiveEnded(u32, String),
    /// The desktop has the screen now, and later gave it back, with why.
    DesktopStarted,
    DesktopEnded(String),
    /// Settings read from /data, which mounts after the panel starts.
    Settings(Settings),
}

#[derive(Clone, Copy, PartialEq)]
enum DesktopMode {
    Off,
    /// The X server is starting; it may already draw, so the panel does not.
    Starting,
    On,
}

/// Makes the desktop's mouse if needed, then starts the desktop in a thread that
/// reports when it runs and when it ends.
fn start_desktop(pointer: &mut Option<Pointer>, format: Format, panel: &mut Ui, input: &mpsc::Sender<PanelInput>) -> DesktopMode {
    if pointer.is_none() {
        match Pointer::create(format.width, format.height) {
            Ok(created) => *pointer = Some(created),
            Err(err) => {
                log(&format!("panel: desktop pointer: {err}"));
                panel.desktop_ended(format!("Not started: the pointer: {err}"));
                return DesktopMode::Off;
            }
        }
    }
    log("panel: starting the desktop");
    let input = input.clone();
    let spawned = thread::Builder::new().spawn(move || {
        let message = match desktop::start(format.width, format.height) {
            Ok(session) => {
                let _ = input.send(PanelInput::DesktopStarted);
                format!("The desktop ended: {}.", session.wait())
            }
            Err(err) => format!("Not started: {err}"),
        };
        let _ = input.send(PanelInput::DesktopEnded(message));
    });
    match spawned {
        Ok(_) => DesktopMode::Starting,
        Err(err) => {
            panel.desktop_ended(format!("Not started: {err}"));
            DesktopMode::Off
        }
    }
}

/// Moves the desktop's mouse. A failure is logged, and the touch is lost.
fn send_pointer(pointer: &mut Option<Pointer>, events: &[input::PointerEvent]) {
    if events.is_empty() {
        return;
    }
    if let Some(pointer) = pointer.as_mut() {
        if let Err(err) = pointer.send(events) {
            log(&format!("panel: desktop pointer: {err}"));
        }
    }
}

/// Live video on the panel ends after this long; LIVE starts it again. Frames keep
/// the screen on, so this is also how long it stays on without a touch.
const PANEL_LIVE_LIMIT: Duration = Duration::from_secs(3 * 60);

/// Live video on the camera page. The camera thread hands each frame to a fitting
/// thread when it is free, which shrinks it for the page and leaves only the
/// newest in `frame`, so a slow panel skips frames instead of falling behind.
struct LiveView {
    number: u32,
    stop: Arc<AtomicBool>,
    frame: Arc<Mutex<Option<photo::Rgb>>>,
    thread: thread::JoinHandle<()>,
    /// Ends after the camera thread; returns the frames it fitted and the time.
    fitter: thread::JoinHandle<(u32, Duration)>,
    started: Instant,
    /// Frames the panel took from `frame` and drew.
    shown: u32,
    /// Panel time while it ran: reading the status, drawing the page, and
    /// copying it to the screen.
    reading: Duration,
    drawing: Duration,
    presenting: Duration,
}

impl LiveView {
    fn start(name: &'static str, number: u32, panel: mpsc::Sender<PanelInput>) -> io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let frame = Arc::new(Mutex::new(None));
        // A rendezvous channel: a frame is only handed over when the fitter waits.
        let (to_fit, frames) = mpsc::sync_channel::<photo::Rgb>(0);
        let (slot, notify) = (frame.clone(), panel.clone());
        let fitter = thread::Builder::new().spawn(move || {
            let (mut fitted, mut fitting) = (0u32, Duration::ZERO);
            for image in frames {
                let t = Instant::now();
                let image = ui::fit_photo(&image);
                fitting += t.elapsed();
                fitted += 1;
                let mut waiting = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                // Only an empty slot needs telling the panel; otherwise it is told already.
                if waiting.replace(image).is_none() && notify.send(PanelInput::LiveFrame(number)).is_err() {
                    break;
                }
            }
            (fitted, fitting)
        })?;
        let stopping = stop.clone();
        let thread = thread::Builder::new().spawn(move || {
            let Some(camera) = camera::by_name(name) else { return };
            log(&format!("camera: {name} live view on the panel"));
            let mut dropped = 0u32;
            let result = camera::live(camera, PANEL_LIVE_LIMIT, |image| {
                if stopping.load(Ordering::Acquire) {
                    return Err(io::Error::from(io::ErrorKind::Interrupted));
                }
                match to_fit.try_send(image) {
                    Ok(()) => Ok(()),
                    Err(mpsc::TrySendError::Full(_)) => {
                        dropped += 1;
                        Ok(())
                    }
                    // The fitter stopped: the panel is gone.
                    Err(mpsc::TrySendError::Disconnected(_)) => Err(io::Error::from(io::ErrorKind::BrokenPipe)),
                }
            });
            drop(to_fit);
            let message = match result {
                Ok(summary) => {
                    let seconds = summary.elapsed.as_secs_f32().max(0.001);
                    log(&format!(
                        "camera: {name} panel live view ended: {} frames {}x{} in {:.0} s ({:.1} fps), \
                         {} sensor frames passed over, {} dropped while fitting; shutter {} gain {:.2}x",
                        summary.sent,
                        summary.width,
                        summary.height,
                        seconds,
                        summary.sent as f32 / seconds,
                        summary.skipped,
                        dropped,
                        summary.exposure.shutter,
                        summary.exposure.gain as f32 / photo::UNIT_GAIN as f32,
                    ));
                    format!("Live video ended after {} min", PANEL_LIVE_LIMIT.as_secs() / 60)
                }
                Err(err) => {
                    log(&format!("camera: {name} panel live view failed: {err}"));
                    format!("Live video failed: {err}")
                }
            };
            // Ignored by the panel once this view was stopped or replaced.
            let _ = panel.send(PanelInput::LiveEnded(number, message));
        });
        let thread = match thread {
            Ok(thread) => thread,
            // `to_fit` went with the closure, so the fitter ends.
            Err(err) => {
                let _ = fitter.join();
                return Err(err);
            }
        };
        Ok(LiveView {
            number,
            stop,
            frame,
            thread,
            fitter,
            started: Instant::now(),
            shown: 0,
            reading: Duration::ZERO,
            drawing: Duration::ZERO,
            presenting: Duration::ZERO,
        })
    }

    /// Stops the view and waits for the camera to be free, which takes up to a
    /// frame and closing the camera, so that another view can start at once.
    fn stop(self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.thread.join();
        let (fitted, fitting) = self.fitter.join().unwrap_or_default();
        let seconds = self.started.elapsed().as_secs_f32().max(0.001);
        let per_frame = |total: Duration, count: u32| total.as_millis() / u128::from(count.max(1));
        log(&format!(
            "panel: live view showed {} frames ({:.1} fps); per frame {} ms fit (own thread), \
             {} ms status, {} ms draw, {} ms present",
            self.shown,
            self.shown as f32 / seconds,
            per_frame(fitting, fitted),
            per_frame(self.reading, self.shown),
            per_frame(self.drawing, self.shown),
            per_frame(self.presenting, self.shown),
        ));
    }
}

/// Acts on the camera page's live video requests, which the panel loop handles
/// itself because the view runs until stopped.
fn live_request(live: &mut Option<LiveView>, count: &mut u32, panel: &mut Ui, request: Request, input: &mpsc::Sender<PanelInput>) {
    if let Some(view) = live.take() {
        view.stop();
    }
    if let Request::Live(name) = request {
        *count = count.wrapping_add(1);
        match LiveView::start(name, *count, input.clone()) {
            Ok(view) => *live = Some(view),
            Err(err) => panel.live_ended(format!("Live video failed: {err}")),
        }
    }
}

/// The shell of the panel's console, on a pseudo-terminal.
struct ConsoleShell {
    master: File,
    child: std::process::Child,
}

impl ConsoleShell {
    /// Starts Alpine's shell, with a thread passing its output to the panel.
    fn start(columns: usize, rows: usize, panel: mpsc::Sender<PanelInput>) -> io::Result<Self> {
        let (master, child) = alpine::start_shell(columns, rows)?;
        let mut output = master.try_clone()?;
        log(&format!("panel: console shell started as {}", child.id()));
        thread::Builder::new().spawn(move || {
            let mut buffer = [0u8; 4096];
            // Reading fails with EIO once the shell and everything it started end.
            while let Ok(length @ 1..) = output.read(&mut buffer) {
                if panel.send(PanelInput::TerminalOutput(buffer[..length].to_vec())).is_err() {
                    return;
                }
            }
            let _ = panel.send(PanelInput::TerminalEnded);
        })?;
        Ok(ConsoleShell { master, child })
    }
}

/// Acts on the console page's requests: they need the shell at once, so they are
/// not queued behind slow work.
fn console_request(shell: &mut Option<ConsoleShell>, panel: &mut Ui, request: Request, input: &mpsc::Sender<PanelInput>) {
    let (columns, rows) = panel.terminal_dimensions();
    if shell.is_none() {
        match ConsoleShell::start(columns, rows, input.clone()) {
            Ok(started) => *shell = Some(started),
            Err(err) => {
                log(&format!("panel: console shell not started: {err}"));
                panel.terminal_note(&format!("[the shell did not start: {err}]"));
                return;
            }
        }
    }
    let Some(running) = shell else { return };
    let result = match request {
        Request::TerminalSize { columns, rows } => alpine::set_terminal_size(&running.master, columns, rows),
        Request::TerminalInput(bytes) => running.master.write_all(&bytes),
        _ => Ok(()),
    };
    // A shell that just ended is reported by its output thread.
    if let Err(err) = result {
        log(&format!("panel: console write failed: {err}"));
    }
}

/// Where `ui tap` and `ui drag` send their touches, set once the panel is running.
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
// The panel's settings, such as how long the screen stays on and how bright it is.
const SETTINGS_FILE: &str = "/data/a32/settings";
// Drawing the whole screen each second took much of the idle CPU time, so by default
// the screen turns off a minute after the last use. Read from /data after it mounts.
static SETTINGS: Mutex<Settings> = Mutex::new(Settings::DEFAULT);
const BATTERY_CAPACITY: &str = "/sys/class/power_supply/battery/capacity";
const BATTERY_STATUS: &str = "/sys/class/power_supply/battery/status";
const BATTERY_MILLIVOLTS: &str = "/sys/class/power_supply/battery/batt_vol";
const CHARGER_MILLIVOLTS: &str = "/sys/class/power_supply/battery/ChargerVoltage";
const CHARGER_ONLINE: &[&str] =
    &["/sys/class/power_supply/usb/online", "/sys/class/power_supply/ac/online"];
const POWER_CHECK: Duration = Duration::from_secs(60);
const POWER_CHECK_EARLY: Duration = Duration::from_secs(5);
// A battery line in the persistent log this often, which records battery life.
const POWER_LOG_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Set while the charger is attached but charging has stopped.
static CHARGING_STALLED: AtomicBool = AtomicBool::new(false);

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

/// Host name for shell prompts and sshd, replacing the kernel's "(none)".
const HOSTNAME: &str = "a32";

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

    // Messages and failures are collected because `/dev/kmsg` does not exist until the
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

    // The kernel starts with "(none)", which shell prompts and sshd would show.
    // SAFETY: the pointer and length describe a valid byte string.
    if unsafe { sethostname(HOSTNAME.as_ptr().cast(), HOSTNAME.len()) } != 0 {
        errors.push(format!("hostname failed: {}", io::Error::last_os_error()));
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
    match start_zram_swap() {
        Ok(size) => errors.push(format!("zram swap: {} MiB", size >> 20)),
        Err(err) => errors.push(format!("zram swap failed: {err}")),
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
    if let Err(err) = thread::Builder::new().spawn(alpine::mdns_service) {
        log(&format!("mdns thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(thermal_service) {
        log(&format!("thermal thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(storage_and_network) {
        log(&format!("network thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(power_service) {
        log(&format!("power thread failed: {err}"));
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

fn read_power() -> charging::Reading {
    let read = |path: &str| fs::read_to_string(path).map(|text| text.trim().to_string()).unwrap_or_default();
    let number = |path: &str| read(path).parse().ok();
    charging::Reading {
        charger: CHARGER_ONLINE.iter().any(|path| read(path) == "1"),
        status: read(BATTERY_STATUS),
        capacity: number(BATTERY_CAPACITY),
        battery_millivolts: number(BATTERY_MILLIVOLTS),
        charger_millivolts: number(CHARGER_MILLIVOLTS),
    }
}

/// Logs the charger and the battery, and powers off cleanly on an empty battery,
/// before the voltage falls far enough to cut power in the middle of a write.
fn power_service() {
    let mut monitor = charging::Monitor::default();
    let mut next_log = Duration::ZERO;
    loop {
        heartbeat(Service::Power);
        let now = since_start();
        let reading = read_power();
        if !reading.is_ready() {
            thread::sleep(POWER_CHECK_EARLY);
            continue;
        }
        let events = monitor.update(now, &reading);
        CHARGING_STALLED.store(monitor.is_stalled(), Ordering::Release);
        for event in &events {
            log(&format!("power: {}: {}", match event {
                PowerEvent::ChargerAttached => "charger attached",
                PowerEvent::ChargerRemoved => "charger removed",
                PowerEvent::Stalled => "charging stopped with the charger attached, plug USB in again",
                PowerEvent::Resumed => "charging again",
                PowerEvent::Empty => "battery empty, powering off",
            }, reading.describe()));
        }
        if events.contains(&PowerEvent::Empty) {
            let err = KernelPlatform.power_off();
            log(&format!("power: power off failed: {err}"));
        }
        if now >= next_log {
            log(&format!("power: {}", reading.describe()));
            next_log = now + POWER_LOG_INTERVAL;
        }
        thread::sleep(POWER_CHECK);
    }
}

/// Serves the status page, one request at a time.
fn web_service() {
    let listener = match TcpListener::bind(("0.0.0.0", web::PORT)) {
        Ok(listener) => listener,
        Err(err) => {
            log(&format!("web: not started: {err}"));
            return;
        }
    };
    log(&format!("web: status page on port {}", web::PORT));
    for connection in listener.incoming() {
        match connection {
            // A client that leaves early or stays silent is normal, so not logged.
            Ok(stream) => {
                let _ = serve_web(stream);
            }
            Err(err) => {
                log(&format!("web: accept failed: {err}"));
                thread::sleep(POLL_INTERVAL);
            }
        }
    }
}

fn serve_web(mut stream: TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(WEB_TIMEOUT))?;
    stream.set_write_timeout(Some(WEB_TIMEOUT))?;
    let mut request = Vec::new();
    let mut buffer = [0u8; 1024];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
        if request.len() >= web::REQUEST_LIMIT {
            let reply = web::response("431 Request Header Fields Too Large", "text/plain; charset=utf-8", "too large\n");
            return stream.write_all(&reply);
        }
        let length = stream.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..length]);
    }
    let plain = "text/plain; charset=utf-8";
    let request = String::from_utf8_lossy(&request);
    let route = web::route(&request);
    if let Route::Photo(_) | Route::Live(_) = route {
        let Some(password) = web_password() else {
            let text = "the cameras are off until a web password is set on the console\n";
            return stream.write_all(&web::response("403 Forbidden", plain, text));
        };
        if !web::authorized(&request, &password) {
            if request.lines().any(|line| line.to_ascii_lowercase().starts_with("authorization:")) {
                log(&format!("web: wrong camera password from {}", peer_text(&stream)));
                thread::sleep(WRONG_PASSWORD_DELAY);
            }
            return stream.write_all(&web::unauthorized());
        }
    }
    let reply = match route {
        Route::Page => {
            let tail = web::last_lines(&log_tail(), WEB_LOG_LINES);
            let sensors = sensor_rows();
            let body = web::page(&snapshot(), &wall_clock_text(), &sensors, &tail);
            web::response("200 OK", "text/html; charset=utf-8", &body)
        }
        Route::Json => web::response("200 OK", "application/json", &web::json(&snapshot(), &wall_clock_text())),
        Route::Bluetooth => {
            let state = bluetooth_state();
            let body = web::bluetooth_json(&state.state, &state.devices.list(), since_start());
            drop(state);
            web::response("200 OK", "application/json", &body)
        }
        // Takes about 2 s, during which other requests wait.
        Route::Sensors => match crate::phone_sensors::read() {
            Ok(reading) => web::response("200 OK", "application/json", &reading.json()),
            Err(err) => web::response("503 Service Unavailable", "text/plain; charset=utf-8", &format!("{err}\n")),
        },
        // Takes a second or two, during which other requests wait.
        Route::Photo(camera) => match take_photo(camera) {
            Ok(photo) => web::response_bytes("200 OK", "image/jpeg", &photo.jpeg),
            Err(err) => web::response("503 Service Unavailable", plain, &format!("camera: {err}\n")),
        },
        // Lasts as long as the viewer watches, so it gets its own thread.
        Route::Live(camera) => {
            thread::spawn(move || serve_live(stream, camera));
            return Ok(());
        }
        Route::NotFound => web::response("404 Not Found", plain, "not found\n"),
        Route::MethodNotAllowed => web::response("405 Method Not Allowed", plain, "only GET\n"),
        Route::BadRequest => web::response("400 Bad Request", plain, "bad request\n"),
    };
    stream.write_all(&reply)
}

/// The camera password, if one is set.
fn web_password() -> Option<String> {
    let text = fs::read_to_string(WEB_PASSWORD).ok()?;
    let password = text.trim_end_matches('\n');
    web::password_problem(password).is_none().then(|| password.to_string())
}

fn peer_text(stream: &TcpStream) -> String {
    stream.peer_addr().map_or_else(|_| "an unknown address".to_string(), |address| address.ip().to_string())
}

/// What the encoding thread of a live view did.
#[derive(Default)]
struct LiveSending {
    frames: u32,
    bytes: usize,
    encoding: Duration,
    sending: Duration,
}

/// Sends live video as MJPEG until the viewer leaves or `LIVE_LIMIT` passes.
/// The camera thread develops frames while a second thread encodes and sends
/// the last one, so two cores work at once; a frame that arrives while the
/// encoder is busy is dropped rather than queued, which keeps the delay short.
fn serve_live(mut stream: TcpStream, name: &'static str) {
    let Some(camera) = camera::by_name(name) else { return };
    let peer = peer_text(&stream);
    let _ = stream.set_nodelay(true);
    let mut output = match stream.try_clone() {
        Ok(output) => output,
        Err(err) => {
            log(&format!("camera: {name} live view failed: {err}"));
            return;
        }
    };
    log(&format!("camera: {name} live view for {peer}"));
    // A rendezvous channel: a frame is only handed over when the encoder waits.
    let (frames, received) = mpsc::sync_channel::<photo::Rgb>(0);
    let encoder = thread::spawn(move || {
        let mut done = LiveSending::default();
        for image in received {
            let t = Instant::now();
            let jpeg = photo::encode_jpeg(&image, LIVE_QUALITY);
            done.encoding += t.elapsed();
            let t = Instant::now();
            if done.frames == 0 && output.write_all(web::live_header()).is_err() {
                break;
            }
            if output.write_all(&web::live_part(&jpeg)).is_err() {
                break;
            }
            done.sending += t.elapsed();
            done.frames += 1;
            done.bytes += jpeg.len();
        }
        done
    });
    let mut dropped = 0u32;
    let result = camera::live(camera, LIVE_LIMIT, |image| match frames.try_send(image) {
        Ok(()) => Ok(()),
        Err(mpsc::TrySendError::Full(_)) => {
            dropped += 1;
            Ok(())
        }
        // The encoder stopped: the viewer left.
        Err(mpsc::TrySendError::Disconnected(_)) => Err(io::Error::from(io::ErrorKind::BrokenPipe)),
    });
    drop(frames);
    let sent = encoder.join().unwrap_or_default();
    match result {
        Ok(summary) => {
            let developed = summary.sent.max(1);
            let per_frame = |total: Duration, count: u32| total.as_millis() / u128::from(count.max(1));
            let seconds = summary.elapsed.as_secs_f32().max(0.001);
            log(&format!(
                "camera: {name} live view ended: sent {} frames {}x{} in {:.0} s ({:.1} fps), {} KB each; \
                 developed {} ({} sensor frames passed over, {} dropped while encoding); per frame {} ms copy, \
                 {} ms develop, {} ms encode, {} ms send; shutter {} gain {:.2}x",
                sent.frames,
                summary.width,
                summary.height,
                seconds,
                sent.frames as f32 / seconds,
                sent.bytes / 1024 / sent.frames.max(1) as usize,
                summary.sent,
                summary.skipped,
                dropped,
                per_frame(summary.copying, developed),
                per_frame(summary.developing, developed),
                per_frame(sent.encoding, sent.frames),
                per_frame(sent.sending, sent.frames),
                summary.exposure.shutter,
                summary.exposure.gain as f32 / photo::UNIT_GAIN as f32,
            ));
        }
        Err(err) => {
            log(&format!("camera: {name} live view failed: {err}"));
            if sent.frames == 0 {
                let reply = web::response("503 Service Unavailable", "text/plain; charset=utf-8", &format!("camera: {err}\n"));
                let _ = stream.write_all(&reply);
            }
        }
    }
}

/// Runs the BLE scan while it is wanted. It starts once the Wi-Fi start has made the
/// device, and a failure is retried, because the chip may still be starting.
fn bluetooth_service() {
    let wanted = DATA_READY.load(Ordering::Acquire)
        && fs::read_to_string(BLUETOOTH_SETTING).is_ok_and(|text| text.trim() == "on");
    let saved = if DATA_READY.load(Ordering::Acquire) {
        sensors::Saved::from_text(&fs::read_to_string(SAVED_SENSORS).unwrap_or_default())
    } else {
        sensors::Saved::default()
    };
    {
        let mut state = bluetooth_state();
        state.wanted = wanted;
        state.state = "off".to_string();
        state.saved = saved;
    }
    loop {
        let starting = fs::read_to_string(WIFI_STATE).is_ok_and(|state| state.trim() == "starting");
        if !bluetooth_state().wanted || starting || !Path::new(BLUETOOTH_DEVICE).exists() {
            thread::sleep(BLUETOOTH_IDLE_CHECK);
            continue;
        }
        let result = run_bluetooth_scan();
        let mut state = bluetooth_state();
        state.devices.clear();
        match result {
            Ok(()) => {
                log("bluetooth: scan stopped");
                state.state = "off".to_string();
            }
            Err(err) => {
                log(&format!("bluetooth: {err}"));
                state.state = format!("failed, retrying: {err}");
                drop(state);
                thread::sleep(BLUETOOTH_RETRY);
            }
        }
    }
}

/// Starts a passive scan and records what it hears until the scan is no longer
/// wanted. Closing the device on return turns Bluetooth off.
fn run_bluetooth_scan() -> io::Result<()> {
    let mut device = OpenOptions::new().read(true).write(true).open(BLUETOOTH_DEVICE)?;
    let mut parser = bluetooth::Parser::default();
    // The driver warns in the kernel log about any read over its 2048 byte buffer, and
    // an event is at most 258 bytes.
    let mut buffer = [0u8; 1024];
    for (opcode, parameters) in bluetooth::start_scan() {
        device.write_all(&bluetooth::command(opcode, &parameters))?;
        let deadline = Instant::now() + BLUETOOTH_REPLY_WAIT;
        let status = loop {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() || !wait_readable(&device, Some(left))? {
                return Err(io::Error::new(io::ErrorKind::TimedOut, format!("no reply to command {opcode:#06x}")));
            }
            let length = device.read(&mut buffer)?;
            let replied = parser.push(&buffer[..length]).into_iter().find_map(|event| match event {
                bluetooth::Event::Complete(done, status) if done == opcode => Some(status),
                _ => None,
            });
            if let Some(status) = replied {
                break status;
            }
        };
        if status != 0 {
            return Err(io::Error::other(format!("command {opcode:#06x} failed with status {status:#04x}")));
        }
    }
    log("bluetooth: scanning");
    bluetooth_state().state = "scanning".to_string();
    let mut next_cleanup = since_start() + bluetooth::FORGET_AFTER;
    let mut last_history = since_start();
    let mut next_history = last_history + SENSOR_HISTORY_INTERVAL;
    while bluetooth_state().wanted {
        if wait_readable(&device, Some(POLL_INTERVAL))? {
            let length = device.read(&mut buffer)?;
            if length == 0 {
                return Err(io::Error::other("the device closed"));
            }
            let now = since_start();
            let events = parser.push(&buffer[..length]);
            let mut state = bluetooth_state();
            for event in events {
                if let bluetooth::Event::Advertisements(heard) = event {
                    for advertisement in &heard {
                        state.devices.record(advertisement, now);
                    }
                }
            }
        }
        let now = since_start();
        if now >= next_cleanup {
            bluetooth_state().devices.forget_old(now);
            next_cleanup = now + bluetooth::FORGET_AFTER;
        }
        if now >= next_history {
            if let Err(err) = save_sensor_history(last_history) {
                log(&format!("bluetooth: sensor history not saved: {err}"));
            }
            last_history = now;
            next_history = now + SENSOR_HISTORY_INTERVAL;
        }
    }
    // Stopped politely, although closing the device turns the radio off anyway.
    device.write_all(&bluetooth::command(bluetooth::LE_SET_SCAN_ENABLE, &[0x00, 0x00]))?;
    Ok(())
}

/// Describes the scan and the strongest devices, for the console.
fn bluetooth_report() -> String {
    let state = bluetooth_state();
    let now = since_start();
    let devices = state.devices.list();
    let mut text = format!(
        "bluetooth: {}, {}\n{} devices heard in the last {} s",
        if state.wanted { "on" } else { "off" },
        state.state,
        devices.len(),
        bluetooth::FORGET_AFTER.as_secs()
    );
    for device in devices.iter().take(BLUETOOTH_LIST_LIMIT) {
        text.push_str(&format!(
            "\n{} {:>4} dBm {:>4} s ago {}{}",
            bluetooth::address_text(&device.address),
            device.rssi,
            now.saturating_sub(device.last_seen).as_secs(),
            if device.random { "random " } else { "" },
            device.label().unwrap_or(""),
        ));
    }
    let sensors = state.devices.sensors();
    text.push_str(&format!("\n{} saved sensors:", state.saved.addresses().len()));
    for saved in state.saved.addresses() {
        let summary = sensors
            .iter()
            .find(|device| device.address == *saved)
            .map_or_else(|| "not heard".to_string(), |device| web::sensor_summary(device, now));
        text.push_str(&format!("\n{} {summary}", bluetooth::address_text(saved)));
    }
    let others: Vec<&&bluetooth::Device> =
        sensors.iter().filter(|device| !state.saved.contains(&device.address)).collect();
    if !others.is_empty() {
        text.push_str(&format!("\n{} other sensors heard, add one with: bluetooth add ADDRESS", others.len()));
        for device in others {
            text.push_str(&format!(
                "\n{} {}",
                bluetooth::address_text(&device.address),
                web::sensor_summary(device, now)
            ));
        }
    }
    text
}

/// A label and a summary for each sensor, for the status page.
fn sensor_rows() -> Vec<(String, String)> {
    let state = bluetooth_state();
    let now = since_start();
    let sensors = state.devices.sensors();
    state
        .saved
        .addresses()
        .iter()
        .map(|saved| {
            let address = bluetooth::address_text(saved);
            match sensors.iter().find(|device| device.address == *saved) {
                Some(device) => {
                    let label = match &device.name {
                        Some(name) => format!("{name} {address}"),
                        None => address,
                    };
                    (label, web::sensor_summary(device, now))
                }
                None => (address, "not heard in the last 10 minutes".to_string()),
            }
        })
        .collect()
}

/// Appends the latest values of each sensor heard since `since` to the history on
/// /data. One older file is kept, so this uses at most twice the limit.
fn save_sensor_history(since: Duration) -> io::Result<()> {
    if !DATA_READY.load(Ordering::Acquire) {
        return Ok(());
    }
    let time = wall_clock_text();
    let state = bluetooth_state();
    let lines: String = state
        .devices
        .sensors()
        .into_iter()
        .filter(|device| state.saved.contains(&device.address))
        .filter_map(|device| {
            let (reading, updated) = device.sensor.as_ref()?;
            (*updated > since)
                .then(|| sensors::csv_line(&time, &bluetooth::address_text(&device.address), reading))
        })
        .collect();
    // Not held while writing, so that the scan does not wait for the eMMC.
    drop(state);
    if lines.is_empty() {
        return Ok(());
    }
    if fs::metadata(SENSOR_HISTORY).is_ok_and(|meta| meta.len() >= SENSOR_HISTORY_LIMIT) {
        fs::rename(SENSOR_HISTORY, SENSOR_HISTORY_OLD)?;
    }
    let new = !Path::new(SENSOR_HISTORY).exists();
    let mut file = OpenOptions::new().create(true).append(true).mode(0o600).open(SENSOR_HISTORY)?;
    if new {
        file.write_all(sensors::CSV_HEADER.as_bytes())?;
    }
    file.write_all(lines.as_bytes())
}

/// The end of the persistent log, or of this session's log if /data is not mounted.
fn log_tail() -> String {
    let path = if DATA_READY.load(Ordering::Acquire) && Path::new(PERSISTENT_LOG).exists() {
        PERSISTENT_LOG
    } else {
        INIT_LOG
    };
    let read = || -> io::Result<String> {
        let mut file = File::open(path)?;
        let length = file.metadata()?.len();
        let start = length.saturating_sub(WEB_LOG_BYTES);
        file.seek(SeekFrom::Start(start))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        // A read from the middle starts inside a line.
        Ok(match text.split_once('\n') {
            Some((_, rest)) if start > 0 => rest.to_string(),
            _ => text,
        })
    };
    read().unwrap_or_else(|err| format!("log not readable: {err}"))
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
    let backlight = fs::write(BACKLIGHT, settings().backlight());
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
    // After `PANEL_INPUT` is set, so that settings read from /data reach the panel
    // either here or as a message.
    panel.set_settings(settings());
    let mut shell: Option<ConsoleShell> = None;
    let mut live: Option<LiveView> = None;
    let mut live_count = 0u32;
    let mut last_use = Instant::now();
    let mut was_on = true;
    // A touch that wakes the screen must not also press a button under the finger.
    let mut waking = false;
    // The desktop's mouse, made once and kept, and the touch being turned into it.
    let mut desktop = DesktopMode::Off;
    let mut pointer: Option<Pointer> = None;
    let mut gesture = Gesture::new(settings().right_click_hold());
    let clock = Instant::now();
    loop {
        heartbeat(Service::Panel);
        if desktop == DesktopMode::On {
            send_pointer(&mut pointer, &gesture.tick(clock.elapsed()));
        }
        let on = SCREEN_ON.load(Ordering::Acquire);
        if on && !was_on {
            // Turned on by the power key.
            last_use = Instant::now();
        }
        if !on && was_on {
            panel.reset();
            live_request(&mut live, &mut live_count, &mut panel, Request::StopLive, &input);
        }
        was_on = on;
        if on && settings().screen_timeout().is_some_and(|timeout| last_use.elapsed() >= timeout) {
            set_screen(false);
            panel.reset();
            live_request(&mut live, &mut live_count, &mut panel, Request::StopLive, &input);
            was_on = false;
        } else if on && desktop == DesktopMode::Off {
            let t = Instant::now();
            let status = snapshot();
            // Taps until the next drawing pick from these, which are what is shown.
            panel.set_sensors(status.bluetooth_saved.clone(), status.bluetooth_candidates.clone());
            let read = Instant::now();
            panel.render(&mut canvas, &status);
            let drawn = Instant::now();
            framebuffer.present(canvas.pixels())?;
            if let Some(view) = live.as_mut() {
                view.reading += read - t;
                view.drawing += drawn - read;
                view.presenting += drawn.elapsed();
            }
        }

        // Redraw when something changes, and otherwise once a second. A finger held
        // on the desktop wakes this sooner, for its right click.
        let wait = gesture.wait_limit(clock.elapsed()).map_or(Duration::from_secs(1), |limit| limit.min(Duration::from_secs(1)));
        if let Ok(first) = inputs.recv_timeout(wait) {
            // Only what is queued now: events that keep arriving, such as live
            // frames, would otherwise hold off drawing for ever.
            let queued: Vec<PanelInput> = std::iter::once(first).chain(inputs.try_iter()).collect();
            for event in queued {
                match event {
                    PanelInput::Touch(position) => {
                        // With touch waking turned off, only the power key turns
                        // the screen on, and touches on a dark screen go nowhere.
                        if !SCREEN_ON.load(Ordering::Acquire) && !settings().touch_wakes() {
                            continue;
                        }
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
                        // While the desktop starts, touches go nowhere.
                        if desktop != DesktopMode::Off {
                            if desktop == DesktopMode::On {
                                send_pointer(&mut pointer, &gesture.touch(position, clock.elapsed()));
                            }
                            continue;
                        }
                        match panel.touch(position) {
                            Some(request @ (Request::TerminalSize { .. } | Request::TerminalInput(_))) => {
                                console_request(&mut shell, &mut panel, request, &input);
                            }
                            Some(Request::StartDesktop) => {
                                desktop = start_desktop(&mut pointer, format, &mut panel, &input);
                            }
                            Some(request @ (Request::Live(_) | Request::StopLive)) => {
                                live_request(&mut live, &mut live_count, &mut panel, request, &input);
                            }
                            // Applied here at once; saving to the eMMC is slow work.
                            Some(Request::Settings(chosen)) => {
                                apply_settings(chosen);
                                let _ = requests.send(Request::Settings(chosen));
                            }
                            Some(request) => {
                                let _ = requests.send(request);
                            }
                            None => {}
                        }
                    }
                    // Output does not keep the screen on: a program like `top`
                    // would never let it turn off.
                    PanelInput::TerminalOutput(bytes) => panel.terminal_output(&bytes),
                    // The desktop's hold time is read when the desktop starts.
                    PanelInput::Settings(loaded) => panel.set_settings(loaded),
                    PanelInput::TerminalEnded => {
                        if let Some(mut ended) = shell.take() {
                            let status = ended.child.wait().map_or_else(|err| err.to_string(), |status| status.to_string());
                            log(&format!("panel: console shell ended: {status}"));
                            panel.terminal_note(&format!("[the shell ended: {status}; type to start a new one]"));
                        }
                    }
                    PanelInput::DesktopStarted => {
                        log("panel: desktop started");
                        desktop = DesktopMode::On;
                        gesture = Gesture::new(settings().right_click_hold());
                    }
                    PanelInput::DesktopEnded(message) => {
                        log(&format!("panel: desktop: {message}"));
                        desktop = DesktopMode::Off;
                        gesture = Gesture::new(settings().right_click_hold());
                        // The X server drew over the panel.
                        framebuffer.forget();
                        last_use = Instant::now();
                        panel.desktop_ended(message);
                    }
                    PanelInput::Networks(networks) => panel.set_networks(networks),
                    PanelInput::Photo(image) => {
                        last_use = Instant::now();
                        panel.set_photo(&image);
                    }
                    PanelInput::Message(message) => {
                        last_use = Instant::now();
                        panel.set_message(message);
                    }
                    // Frames from a view already stopped or replaced are dropped.
                    PanelInput::LiveFrame(number) => {
                        let Some(view) = live.as_mut().filter(|view| view.number == number) else { continue };
                        let image = view.frame.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
                        if let Some(image) = image {
                            last_use = Instant::now();
                            panel.show_live(image);
                            view.shown += 1;
                        }
                    }
                    PanelInput::LiveEnded(number, message) => {
                        if live.as_ref().is_some_and(|view| view.number == number) {
                            live_request(&mut live, &mut live_count, &mut panel, Request::StopLive, &input);
                            last_use = Instant::now();
                            panel.live_ended(message);
                        }
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
        battery: if CHARGING_STALLED.load(Ordering::Acquire) {
            format!("{}% NOT CHARGING", read(BATTERY_CAPACITY).trim())
        } else {
            format!("{}% {}", read(BATTERY_CAPACITY).trim(), read(BATTERY_STATUS).trim())
        },
        battery_temperature: degrees(BATTERY_ZONE),
        bluetooth: {
            let state = bluetooth_state();
            if state.state == "scanning" {
                format!("scanning, {} devices heard", state.devices.list().len())
            } else {
                state.state.clone()
            }
        },
        bluetooth_saved: saved_sensor_rows(),
        bluetooth_candidates: {
            let state = bluetooth_state();
            // Readable sensors first, then encrypted ones, listed so that the page can
            // say why they cannot be added.
            let readable = state.devices.sensors().into_iter();
            let encrypted = state.devices.list().into_iter().filter(|device| device.encrypted && device.sensor.is_none());
            readable
                .chain(encrypted)
                .filter(|device| !state.saved.contains(&device.address))
                .map(|device| {
                    let address = bluetooth::address_text(&device.address);
                    let values = device
                        .sensor
                        .as_ref()
                        .map_or_else(|| "encrypted".to_string(), |(reading, _)| sensors::short_values(reading));
                    // The address too, because two sensors of one model share a name.
                    let line = match &device.name {
                        Some(name) => format!("{name} {} {values}", &address[9..]),
                        None => format!("{address} {values}"),
                    };
                    SensorRow { line, address, readable: device.sensor.is_some() }
                })
                .collect()
        },
        bluetooth_nearby: {
            let state = bluetooth_state();
            state
                .devices
                .list()
                .iter()
                .take(BLUETOOTH_PANEL_NEARBY)
                .map(|device| match device.label() {
                    Some(label) => format!("{:>4}  {label}", device.rssi),
                    None => format!("{:>4}  {}", device.rssi, bluetooth::address_text(&device.address)),
                })
                .collect()
        },
    }
}

/// The saved sensors as panel rows, in the order they were added, with their latest
/// values or `not heard`.
fn saved_sensor_rows() -> Vec<SensorRow> {
    let state = bluetooth_state();
    let sensors = state.devices.sensors();
    state
        .saved
        .addresses()
        .iter()
        .map(|saved| {
            let address = bluetooth::address_text(saved);
            let device = sensors.iter().find(|device| device.address == *saved);
            let values = device
                .and_then(|device| device.sensor.as_ref())
                .map_or_else(|| "not heard".to_string(), |(reading, _)| sensors::short_values(reading));
            let line = match device.and_then(|device| device.name.as_deref()) {
                Some(name) => format!("{name} {values}"),
                None => format!("{address} {values}"),
            };
            SensorRow { line, address, readable: true }
        })
        .collect()
}

/// Saves or removes a sensor in the list kept on /data.
fn change_saved_sensor(add: bool, text: &str) -> io::Result<String> {
    let address = bluetooth::parse_address(text)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "an address looks like 58:2d:34:36:74:93"))?;
    if !DATA_READY.load(Ordering::Acquire) {
        return Err(io::Error::other("/data is not mounted, so the list could not be kept"));
    }
    let encrypted = bluetooth_state()
        .devices
        .list()
        .iter()
        .any(|device| device.address == address && device.encrypted && device.sensor.is_none());
    if add && encrypted {
        return Err(io::Error::other(format!(
            "{} sends encrypted values, which cannot be read here",
            bluetooth::address_text(&address)
        )));
    }
    // Copied out, so that the scan does not wait for the eMMC.
    let mut saved = bluetooth_state().saved.clone();
    let changed = if add { saved.add(address).map_err(io::Error::other)? } else { saved.remove(&address) };
    let name = bluetooth::address_text(&address);
    if !changed {
        return Ok(format!("{name} was {} saved", if add { "already" } else { "not" }));
    }
    write_private_file(SAVED_SENSORS, &saved.to_text())?;
    bluetooth_state().saved = saved;
    log(&format!("bluetooth: {} sensor {name}", if add { "saved" } else { "removed" }));
    Ok(format!("{} {name}", if add { "saved" } else { "removed" }))
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
            Request::AddSensor(address) => match change_saved_sensor(true, &address) {
                Ok(text) => say(text),
                Err(err) => say(format!("Not saved: {err}")),
            },
            Request::RemoveSensor(address) => match change_saved_sensor(false, &address) {
                Ok(text) => say(text),
                Err(err) => say(format!("Not removed: {err}")),
            },
            // Handled by the panel loop, which owns the shell; never sent here.
            Request::TerminalSize { .. }
            | Request::TerminalInput(_)
            | Request::Live(_)
            | Request::StopLive
            | Request::StartDesktop => {}
            Request::Settings(chosen) => {
                if let Err(err) = save_settings(&chosen) {
                    say(format!("Applied, not kept: {err}"));
                }
            }
            Request::Photo(camera) => match take_photo(camera) {
                Ok(photo) => {
                    say(format!("{camera}: {:.1} s, level {:.0}", photo.elapsed.as_secs_f32(), photo.mean_level));
                    let _ = replies.send(PanelInput::Photo(photo.image));
                }
                Err(err) => say(format!("Photo failed: {err}")),
            },
            Request::Bluetooth(on) => match KernelPlatform.bluetooth(Some(on)) {
                Ok(report) => say(report.lines().next().unwrap_or_default().to_string()),
                Err(err) => say(format!("Bluetooth failed: {err}")),
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

fn settings() -> Settings {
    *SETTINGS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Puts settings in use at once: the backlight changes if the screen is on, the
/// time zone holds for programs started in Alpine from now on, and
/// the rest is read where it is needed.
fn apply_settings(chosen: Settings) {
    alpine::set_time_zone(chosen.tz());
    let before = std::mem::replace(&mut *SETTINGS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()), chosen);
    if before.backlight() != chosen.backlight() && SCREEN_ON.load(Ordering::Acquire) {
        if let Err(err) = fs::write(BACKLIGHT, chosen.backlight()) {
            log(&format!("backlight failed: {err}"));
        }
    }
}

fn save_settings(chosen: &Settings) -> io::Result<()> {
    if !DATA_READY.load(Ordering::Acquire) {
        return Err(io::Error::other("/data is not mounted"));
    }
    write_private_file(SETTINGS_FILE, &chosen.to_text())?;
    log(&format!("panel: settings kept: {}", chosen.to_text().trim_end().replace('\n', ", ")));
    Ok(())
}

/// Reads the settings once /data is mounted, and hands them to the panel. They are
/// stored before being sent, so a panel that starts later reads them itself.
fn load_settings() {
    let loaded = match fs::read_to_string(SETTINGS_FILE) {
        Ok(text) => Settings::from_text(&text),
        Err(err) if err.kind() == io::ErrorKind::NotFound => return,
        Err(err) => {
            log(&format!("settings: not read, using the defaults: {err}"));
            return;
        }
    };
    apply_settings(loaded);
    log(&format!("settings: {}", loaded.to_text().trim_end().replace('\n', ", ")));
    if let Some(panel) = PANEL_INPUT.get() {
        let _ = panel.send(PanelInput::Settings(loaded));
    }
}

fn set_screen(on: bool) {
    SCREEN_ON.store(on, Ordering::Release);
    let level = if on { settings().backlight() } else { "0".to_string() };
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
        Ok(()) => {
            log("firewall: ready");
            // Only behind the firewall, like SSH.
            if let Err(err) = thread::Builder::new().spawn(web_service) {
                log(&format!("web: thread failed: {err}"));
            }
        }
        Err(err) => log(&format!("firewall: failed: {err}")),
    }
    match mount_data() {
        Ok(()) => {
            DATA_READY.store(true, Ordering::Release);
            log("data: mounted");
            if let Err(err) = start_persistent_log() {
                log(&format!("log: not kept across restarts: {err}"));
            }
            load_settings();
            if fs::read_to_string(USB_NETWORK_SETTING).is_ok_and(|text| text.trim() == "on") {
                schedule_usb_network(true);
            }
            if let Err(err) = crate::audio::install() {
                log(&format!("audio: scripts not installed: {err}"));
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
    // It waits for the Wi-Fi start, which brings up the chip Bluetooth shares.
    if let Err(err) = thread::Builder::new().spawn(bluetooth_service) {
        log(&format!("bluetooth: thread failed: {err}"));
    }
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
    drop(state);
    announce_address(interface, lease.address);
    Ok(format!(
        "address {}/{prefix} router {:?} dns {:?} lease {:?} s, renewing in {renew_seconds} s",
        lease.address, lease.router, lease.dns, lease.lease_seconds
    ))
}

/// Sends gratuitous ARP for the address from a short-lived thread, so that a lease
/// renewal does not wait for it. After a move to another access point, hosts on the
/// LAN could not reach the phone for minutes, while the router could.
fn announce_address(interface: &str, address: Ipv4Addr) {
    let interface = interface.to_string();
    let spawned = thread::Builder::new().name("arp-announce".into()).spawn(move || {
        let mut sent = 0;
        let mut last_error = None;
        for gap in ARP_ANNOUNCE_GAPS {
            thread::sleep(gap);
            match send_gratuitous_arp(&interface, address) {
                Ok(()) => sent += 1,
                Err(err) => last_error = Some(err),
            }
        }
        match last_error {
            None => log(&format!("arp: announced {address} on {interface} {sent} times")),
            Some(err) => log(&format!("arp: {sent} of {} announcements sent: {err}", ARP_ANNOUNCE_GAPS.len())),
        }
    });
    if let Err(err) = spawned {
        log(&format!("arp: could not start the announcement: {err}"));
    }
}

fn send_gratuitous_arp(interface: &str, address: Ipv4Addr) -> io::Result<()> {
    let mac_text = fs::read_to_string(format!("/sys/class/net/{interface}/address"))?;
    let mac = dhcp::parse_mac(&mac_text)
        .ok_or_else(|| io::Error::other(format!("bad MAC address: {mac_text}")))?;
    let name = cstring(interface)?;
    // SAFETY: `name` is a valid NUL-terminated string for the call duration.
    let index = unsafe { if_nametoindex(name.as_ptr()) };
    if index == 0 {
        return Err(io::Error::last_os_error());
    }
    let frame = wifi::gratuitous_arp(mac, address.octets());
    let mut target = SockaddrLl {
        family: AF_PACKET as u16,
        protocol: wifi::ETHERTYPE_ARP.to_be(),
        interface_index: index as c_int,
        hardware_type: ARP_HARDWARE_ETHER,
        packet_type: 0,
        address_length: 6,
        address: [0; 8],
    };
    target.address[..6].copy_from_slice(&frame[0..6]);

    // Protocol 0: this socket only sends, so it should receive nothing.
    // SAFETY: plain constants; the result is checked below.
    let fd = unsafe { socket(AF_PACKET, SOCK_RAW, 0) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: `frame` and `target` outlive the call, and the lengths match them.
    let written = unsafe {
        sendto(
            fd,
            frame.as_ptr().cast(),
            frame.len(),
            0,
            (&target as *const SockaddrLl).cast(),
            std::mem::size_of::<SockaddrLl>() as c_uint,
        )
    };
    let result = if written == frame.len() as isize {
        Ok(())
    } else if written < 0 {
        Err(io::Error::last_os_error())
    } else {
        Err(io::Error::other(format!("only {written} bytes sent")))
    };
    // SAFETY: `fd` is a socket this function opened and nothing else uses.
    unsafe { close(fd) };
    result
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

/// Leaves the current network, giving the deauthentication time to go out first.
///
/// `remove_network` straight before `scan` moved the radio off the channel while
/// the frame may still have been queued. The old access point then kept the phone
/// as its client: on 2026-11-07, after a move from a TP-Link to another access point
/// on the same LAN, hosts behind the TP-Link could not reach the phone for minutes,
/// while the router could. A gratuitous ARP did not change that.
fn leave_network() -> io::Result<()> {
    let connected = || wifi::wpa_state(&wpa_cli(&["status"]).unwrap_or_default()) == Some(WPA_COMPLETED);
    if connected() {
        wpa_command(&["disconnect"])?;
        if !wait_until(WIFI_LEAVE_WAIT, || !connected()) {
            log("wifi: still connected after disconnect");
        }
        thread::sleep(WIFI_DEAUTH_SETTLE);
        log("wifi: left the network");
    }
    wpa_command(&["remove_network", "all"])
}

fn join_network(credentials: &wifi::Credentials) -> io::Result<()> {
    leave_network()?;
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

pub(crate) fn find_input_device(wanted: &str) -> io::Result<(u32, u32)> {
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

    fn touch(&self, path: &[(i32, i32)]) -> io::Result<()> {
        let panel = PANEL_INPUT.get().ok_or_else(|| io::Error::other("the panel is not running"))?;
        for position in path.iter().copied().map(Some).chain([None]) {
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
        leave_network()?;
        let mut state = lease_state();
        state.renew_at = None;
        state.applied = None;
        run_ip(&["addr", "flush", "dev", WIFI_INTERFACE])?;
        Ok("left the network and erased its saved details".to_string())
    }

    fn dhcp(&self, interface: &str) -> io::Result<String> {
        renew_lease(interface)
    }

    fn bluetooth(&self, switch: Option<bool>) -> io::Result<String> {
        if let Some(on) = switch {
            if !DATA_READY.load(Ordering::Acquire) {
                return Err(io::Error::other("/data is not mounted, so the choice could not be kept"));
            }
            keep_switch(BLUETOOTH_SETTING, on)?;
            bluetooth_state().wanted = on;
            log(&format!("bluetooth: turned {}", if on { "on" } else { "off" }));
            // The scan starts or stops within a couple of seconds.
            thread::sleep(BLUETOOTH_IDLE_CHECK + POLL_INTERVAL);
        }
        Ok(bluetooth_report())
    }

    fn bluetooth_sensor(&self, add: bool, address: &str) -> io::Result<String> {
        change_saved_sensor(add, address)
    }

    fn phone_sensors(&self) -> io::Result<String> {
        crate::phone_sensors::read().map(|reading| reading.text())
    }

    fn usb_network(&self, switch: Option<bool>) -> io::Result<String> {
        if let Some(on) = switch {
            if !DATA_READY.load(Ordering::Acquire) {
                return Err(io::Error::other("/data is not mounted, so the choice could not be kept"));
            }
            keep_switch(USB_NETWORK_SETTING, on)?;
            schedule_usb_network(on);
            return Ok(format!(
                "usb network: turning {}; the serial port reconnects in a moment",
                if on { "on" } else { "off" }
            ));
        }
        Ok(usb_network_report())
    }

    fn photo(&self, camera: &str, path: &str) -> io::Result<String> {
        // Like `recv`, never replace an existing file.
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        let photo = take_photo(camera)?;
        file.write_all(&photo.jpeg)?;
        Ok(format!("{}, saved to {path}", describe_photo(camera, &photo)))
    }

    fn web_password(&self, password: Option<&str>) -> io::Result<String> {
        match password {
            None => {}
            Some(_) if !DATA_READY.load(Ordering::Acquire) => {
                return Err(io::Error::other("/data is not mounted, so the password could not be kept"));
            }
            Some("off") => {
                match fs::remove_file(WEB_PASSWORD) {
                    Err(err) if err.kind() != io::ErrorKind::NotFound => return Err(err),
                    _ => File::open(SETTINGS_DIR)?.sync_all()?,
                }
                log("web: camera password removed, cameras off");
            }
            Some(password) => {
                if let Some(problem) = web::password_problem(password) {
                    return Err(io::Error::new(io::ErrorKind::InvalidInput, problem));
                }
                write_private_file(WEB_PASSWORD, &format!("{password}\n"))?;
                log("web: camera password set");
            }
        }
        Ok(if web_password().is_some() {
            "the web cameras are on, with a password (any user name)".to_string()
        } else {
            "the web cameras are off: no password is set".to_string()
        })
    }

    fn power_off(&self) -> io::Error {
        // SAFETY: `sync` takes no arguments and has no memory safety requirements.
        unsafe { sync() };
        // SAFETY: this reboot command takes no pointer arguments.
        unsafe { syscall(SYS_REBOOT, REBOOT_MAGIC1, REBOOT_MAGIC2, REBOOT_CMD_POWER_OFF, 0) };
        io::Error::last_os_error()
    }
}

/// JPEG quality for photos. At 85 a rear photo took 47 KB to 660 KB in tests; noise
/// from high gain in dim light makes it larger.
const PHOTO_QUALITY: u32 = 85;

/// Takes a photo with the named camera ("rear" or "front") and logs the result.
pub(crate) fn take_photo(name: &str) -> io::Result<camera::Photo> {
    let camera = camera::by_name(name)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("no camera named {name}")))?;
    match camera::take(camera, PHOTO_QUALITY) {
        Ok(photo) => {
            log(&format!("camera: {}", describe_photo(name, &photo)));
            Ok(photo)
        }
        Err(err) => {
            log(&format!("camera: {name} photo failed: {err}"));
            Err(err)
        }
    }
}

fn describe_photo(name: &str, photo: &camera::Photo) -> String {
    format!(
        "{name} photo {}x{}, {} bytes, shutter {} lines, gain {:.2}x, level {:.0}, {} frames in {} ms",
        photo.width,
        photo.height,
        photo.jpeg.len(),
        photo.exposure.shutter,
        photo.exposure.gain as f32 / photo::UNIT_GAIN as f32,
        photo.mean_level,
        photo.frames,
        photo.elapsed.as_millis()
    )
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

/// Keeps an on or off choice as a file that exists only while it is on.
fn keep_switch(path: &str, on: bool) -> io::Result<()> {
    if on {
        return write_private_file(path, "on\n");
    }
    match fs::remove_file(path) {
        Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
        _ => File::open(SETTINGS_DIR)?.sync_all(),
    }
}

fn usb_network_report() -> String {
    let read = |path: String| fs::read_to_string(path).map_or_else(|err| err.to_string(), |text| text.trim().to_owned());
    let running = USB_NETWORK.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).is_some();
    format!(
        "usb network: {}\nphone: {}\ncomputer: {}\nfunctions: {}\nusb state: {}",
        if running { "on" } else { "off" },
        usb::PHONE_ADDRESS,
        usb::COMPUTER_ADDRESS,
        read(format!("{}/functions", usb::ANDROID_USB)),
        read(USB_STATE.to_owned()),
    )
}

/// Changes the USB gadget on its own thread, because the change disconnects the
/// serial console, which may be the one that asked. The console always starts on
/// the serial-only gadget, so that a fault here cannot take away the way in.
fn schedule_usb_network(on: bool) {
    let spawned = thread::Builder::new().spawn(move || {
        let start = Instant::now();
        while !CONSOLE_READY.load(Ordering::Acquire) {
            if start.elapsed() > USB_CONSOLE_WAIT {
                log("usb network: the console did not start, so nothing was changed");
                return;
            }
            thread::sleep(USB_CHECK);
        }
        thread::sleep(USB_SWITCH_DELAY);
        match set_usb_network(on) {
            Ok(()) => log(&format!("usb network: {}", if on { "on" } else { "off" })),
            Err(err) => log(&format!("usb network: failed: {err}")),
        }
    });
    if let Err(err) = spawned {
        log(&format!("usb network: thread failed: {err}"));
    }
}

fn set_usb_network(on: bool) -> io::Result<()> {
    let mut current = USB_NETWORK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if on == current.is_some() {
        return Ok(());
    }
    let root = Path::new(usb::ANDROID_USB);
    if let Some(network) = current.take() {
        network.running.store(false, Ordering::Release);
        let _ = network.server.join();
        return usb::enable_serial(root);
    }
    match usb::enable_serial_and_network(root).and_then(|()| start_usb_network()) {
        Ok(network) => {
            *current = Some(network);
            Ok(())
        }
        Err(err) => {
            if let Err(restore) = usb::enable_serial(root) {
                log(&format!("usb network: could not restore the serial port: {restore}"));
            }
            Err(err)
        }
    }
}

/// Gives the new interface its address and starts answering DHCP on it.
fn start_usb_network() -> io::Result<UsbNetwork> {
    let interface = format!("/sys/class/net/{}", usb::NETWORK_INTERFACE);
    let start = Instant::now();
    while !Path::new(&interface).exists() {
        if start.elapsed() > USB_INTERFACE_WAIT {
            return Err(io::Error::new(io::ErrorKind::TimedOut, format!("no {interface}")));
        }
        thread::sleep(USB_CHECK);
    }
    let address = format!("{}/{}", usb::PHONE_ADDRESS, usb::PREFIX_LENGTH);
    run_ip(&["addr", "add", &address, "dev", usb::NETWORK_INTERFACE])?;
    run_ip(&["link", "set", usb::NETWORK_INTERFACE, "up"])?;

    let socket = UdpSocket::bind(("0.0.0.0", DHCP_SERVER_PORT))?;
    socket.set_broadcast(true)?;
    socket.set_read_timeout(Some(USB_LEASE_WAIT))?;
    bind_to_device(&socket, usb::NETWORK_INTERFACE)?;

    let running = Arc::new(AtomicBool::new(true));
    let flag = Arc::clone(&running);
    let server = thread::Builder::new().spawn(move || serve_usb_leases(socket, &flag))?;
    Ok(UsbNetwork { running, server })
}

fn serve_usb_leases(socket: UdpSocket, running: &AtomicBool) {
    let lease = dhcp::ServerLease {
        server: usb::PHONE_ADDRESS,
        client: usb::COMPUTER_ADDRESS,
        netmask: usb::NETMASK,
        seconds: USB_LEASE_SECONDS,
    };
    let mut buffer = [0u8; 1500];
    while running.load(Ordering::Acquire) {
        let length = match socket.recv_from(&mut buffer) {
            Ok((length, _)) => length,
            Err(err) if matches!(err.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => continue,
            Err(err) => {
                log(&format!("usb network: dhcp read failed: {err}"));
                thread::sleep(USB_LEASE_WAIT);
                continue;
            }
        };
        if let Some(reply) = dhcp::serve(&buffer[..length], &lease) {
            if let Err(err) = socket.send_to(&reply, ("255.255.255.255", DHCP_CLIENT_PORT)) {
                log(&format!("usb network: dhcp reply failed: {err}"));
            }
        }
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

/// Makes half of RAM a compressed swap device. Errors are returned because the
/// log does not exist yet when this runs.
fn start_zram_swap() -> io::Result<u64> {
    let size = crate::swap::device_size(&fs::read_to_string(MEMINFO)?)
        .ok_or_else(|| io::Error::other("no MemTotal in /proc/meminfo"))?;
    let dev = fs::read_to_string(format!("{ZRAM_SYS}/dev"))?;
    let (major, minor) =
        usb::parse_dev(&dev).ok_or_else(|| io::Error::other(format!("bad device number: {dev}")))?;
    make_device(ZRAM_DEVICE, S_IFBLK | 0o600, major, minor)?;
    // The size can only be set once, until the device is reset.
    fs::write(format!("{ZRAM_SYS}/disksize"), size.to_string())?;
    let header = crate::swap::header(size).ok_or_else(|| io::Error::other("zram device too small"))?;
    let mut device = OpenOptions::new().write(true).open(ZRAM_DEVICE)?;
    device.write_all(&header)?;
    device.sync_all()?;
    drop(device);
    let path = cstring(ZRAM_DEVICE)?;
    // SAFETY: `path` is a valid NUL-terminated string for the call duration.
    if unsafe { swapon(path.as_ptr(), crate::swap::swapon_flags(crate::swap::PRIORITY)) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(size)
}

pub(crate) fn make_char_device(path: &str, major: u32, minor: u32, perm: c_uint) -> io::Result<()> {
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
