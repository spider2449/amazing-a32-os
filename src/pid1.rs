//! PID 1 duties: filesystems, device nodes, logging, USB console, status screen
//! and child reaping.
//!
//! The stock 3.18 kernel has no devtmpfs, so `/dev` is a tmpfs populated by hand.
//! PID 1 must never exit, because that panics the kernel and reboots the phone.
//!
//! The phone can only reach fastboot from software, because no key combination
//! does it. So if this process panics or the console cannot start, it reboots to
//! fastboot, where the system can be repaired. A kernel panic is not covered.

use crate::dhcp::{self, MessageType};
use crate::display::{status_lines, Canvas};
use crate::fb::Framebuffer;
use crate::input::{self, Action, PowerKey, Touch, EVENT_LEN};
use crate::shell::{self, Platform};
use crate::usb;
use crate::wifi;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Read, Write};
use std::net::{Ipv4Addr, UdpSocket};
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::os::raw::{c_char, c_int, c_long, c_short, c_uint, c_ulong, c_void};
use std::path::Path;
use std::process::{Command, Stdio};
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
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

const S_IFCHR: c_uint = 0o020000;
const S_IFBLK: c_uint = 0o060000;
const SERIAL_DEVICE: &str = "/dev/ttyGS0";

// Partitions exposed as /dev/NAME so that the console can back them up. Nothing
// in this system writes to them.
const SYS_BLOCK: &str = "/sys/class/block";
const EXPOSED_PARTITIONS: &[&str] = &["boot", "recovery", "system"];
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
const ECHILD: i32 = 10;
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

/// Set when the console device exists. A host does not need to be attached, so
/// running on battery without USB does not count as a failure.
static CONSOLE_READY: AtomicBool = AtomicBool::new(false);

/// A fault to inject for testing the fail-safe, chosen at build time with the
/// `A32_FAULT` environment variable: `panic`, `usb` or `hang`.
const FAULT: Option<&str> = option_env!("A32_FAULT");

const FB_SYSFS_DEV: &str = "/sys/class/graphics/fb0/dev";
const FB_DEVICE: &str = "/dev/fb0";
const DISPLAY_INFO: &str = "/run/display-info";
const BACKLIGHT: &str = "/sys/class/leds/lcd-backlight/brightness";
// A low level, because the screen stays on for the whole session.
const BACKLIGHT_LEVEL: &str = "64";
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
    ("proc", "/proc"),
    ("sysfs", "/sys"),
    ("tmpfs", "/run"),
];

/// Character devices as (path, major, minor, permissions).
const DEVICES: &[(&str, u32, u32, c_uint)] = &[
    ("/dev/null", 1, 3, 0o666),
    ("/dev/kmsg", 1, 11, 0o600),
    ("/dev/console", 5, 1, 0o600),
    ("/dev/zero", 1, 5, 0o666),
    ("/dev/random", 1, 8, 0o666),
    ("/dev/urandom", 1, 9, 0o666),
];

/// Boots userspace. Never returns.
pub fn run() -> ! {
    // The hook never returns, which stops panic=abort from killing PID 1.
    std::panic::set_hook(Box::new(|info| fail_safe(&format!("panic: {info}"))));

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
    if let Err(err) = write_property_area() {
        errors.push(format!("property area failed: {err}"));
    }

    for error in &errors {
        log(error);
    }
    log(&crate::probe::report());

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
    if let Err(err) = thread::Builder::new().spawn(wifi_service) {
        log(&format!("wifi thread failed: {err}"));
    }

    log("boot complete");
    if FAULT == Some("panic") {
        panic!("injected fault");
    }
    reap_forever()
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
    if let Err(err) = run_status_screen() {
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

    // `touches` stays alive here, so the receiver below never sees a disconnect
    // even if the touch thread stops.
    let (touches, touch_updates) = mpsc::channel();
    let sender = touches.clone();
    if let Err(err) = thread::Builder::new().spawn(move || touch_screen(sender)) {
        log(&format!("touch thread failed: {err}"));
    }

    let boot_mode = fs::read_to_string(BOOT_MODE).ok();
    let mut canvas = Canvas::new(format);
    let mut touch = None;
    loop {
        let uptime = fs::read_to_string("/proc/uptime")?;
        let capacity = fs::read_to_string(BATTERY_CAPACITY).ok();
        let status = fs::read_to_string(BATTERY_STATUS).ok();
        let lines = status_lines(
            &uptime,
            capacity.as_deref(),
            status.as_deref(),
            boot_mode.as_deref(),
            touch,
        );
        canvas.render_status(&lines, touch);
        framebuffer.present(canvas.pixels())?;

        // Redraw when the touch changes, and otherwise once a second. Only the
        // newest update matters if drawing fell behind.
        if let Ok(update) = touch_updates.recv_timeout(Duration::from_secs(1)) {
            touch = update;
            touch = touch_updates.try_iter().last().unwrap_or(touch);
        }
    }
}

/// Sends touch updates to the status screen. A failure leaves the rest running.
fn touch_screen(updates: mpsc::Sender<Option<(i32, i32)>>) {
    if let Err(err) = run_touch_screen(&updates) {
        log(&format!("touch stopped: {err}"));
    }
}

fn run_touch_screen(updates: &mpsc::Sender<Option<(i32, i32)>>) -> io::Result<()> {
    let (major, minor) = wait_for_touch_device()?;
    make_char_device(TOUCH_DEVICE, major, minor, 0o600)?;
    let mut device = File::open(TOUCH_DEVICE)?;
    let mut touch = Touch::default();
    loop {
        let mut raw = [0u8; EVENT_LEN];
        device.read_exact(&mut raw)?;
        if let Some(update) = touch.event(&raw) {
            updates.send(update).map_err(io::Error::other)?;
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
    let mut screen_on = true;
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
            Some(Action::ToggleScreen) => {
                screen_on = !screen_on;
                let level = if screen_on { BACKLIGHT_LEVEL } else { "0" };
                if let Err(err) = fs::write(BACKLIGHT, level) {
                    log(&format!("backlight failed: {err}"));
                }
            }
            Some(Action::PowerOff) => {
                log("power key: powering off");
                let err = KernelPlatform.power_off();
                log(&format!("power off failed: {err}"));
            }
            None => {}
        }
    }
}

/// Starts Wi-Fi as far as `wpa_supplicant`. A failure leaves the rest running, and
/// joining a network is left to the console.
fn wifi_service() {
    if fs::read_to_string(BOOT_MODE).is_ok_and(|mode| mode.trim() == LOW_BATTERY_BOOT_MODE) {
        set_wifi_state("off: low battery");
        return;
    }
    set_wifi_state("starting");
    let start = Instant::now();
    match start_wifi() {
        Ok(()) => {
            log(&format!("wifi: started in {} s", start.elapsed().as_secs()));
            set_wifi_state(WIFI_READY);
        }
        Err(err) => set_wifi_state(&format!("failed: {err}")),
    }
}

fn set_wifi_state(state: &str) {
    log(&format!("wifi: {state}"));
    let _ = fs::write(WIFI_STATE, format!("{state}\n"));
}

/// Runs the steps the stock system runs, in the order that was tested by hand.
fn start_wifi() -> io::Result<()> {
    fs::write(FIRMWARE_PATH_PARAMETER, FIRMWARE_DIR)?;
    if !Path::new("/etc").exists() {
        std::os::unix::fs::symlink("/system/etc", "/etc")?;
    }

    make_wmt_devices()?;
    wait_for_child(logged_command(WMT_LOADER, WMT_LOADER_LOG)?.status())?;
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
fn stock_command(program: &str) -> Command {
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

/// `reap_forever` may collect a child first, so that error means it has exited.
fn wait_for_child(result: io::Result<std::process::ExitStatus>) -> io::Result<()> {
    match result {
        Err(err) if err.raw_os_error() != Some(ECHILD) => Err(err),
        _ => Ok(()),
    }
}

/// Runs `wpa_cli` directly, without a shell, so that a passphrase needs no
/// escaping. Its arguments are not logged for the same reason.
fn wpa_cli(arguments: &[&str]) -> io::Result<String> {
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
    wait_for_child(child.wait())?;
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
        wait_for_child(
            stock_command(STOCK_SHELL)
                .args(["-c", line])
                .stdout(output.try_clone()?)
                .stderr(output)
                .status(),
        )?;
        fs::read(STOCK_SHELL_OUTPUT)
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
        let result = join_network(credentials).and_then(|()| self.dhcp(WIFI_INTERFACE));
        if result.is_err() {
            // Also clears the passphrase from `wpa_supplicant`, and stops its retries.
            let _ = wpa_command(&["remove_network", "all"]);
        }
        result
    }

    fn dhcp(&self, interface: &str) -> io::Result<String> {
        let lease = acquire_lease(interface)?;
        let netmask = lease.netmask.unwrap_or(Ipv4Addr::new(255, 255, 255, 0));
        let prefix = dhcp::prefix_length(netmask)
            .ok_or_else(|| io::Error::other(format!("bad netmask: {netmask}")))?;

        run_ip(&["addr", "flush", "dev", interface])?;
        run_ip(&["addr", "add", &format!("{}/{prefix}", lease.address), "dev", interface])?;
        if let Some(router) = lease.router {
            run_ip(&["route", "replace", "default", "via", &router.to_string(), "dev", interface])?;
        }
        Ok(format!(
            "address {}/{prefix} router {:?} dns {:?} lease {:?} s (not renewed)",
            lease.address, lease.router, lease.dns, lease.lease_seconds
        ))
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
    let status = Command::new(STOCK_IP)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(status) if !status.success() => {
            Err(io::Error::other(format!("ip {arguments:?} failed: {status}")))
        }
        // `reap_forever` may collect the child first, which hides its exit status.
        Err(err) if err.raw_os_error() != Some(ECHILD) => Err(err),
        _ => Ok(()),
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

fn cstring(value: &str) -> io::Result<CString> {
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
fn log(message: &str) {
    if let Ok(mut kmsg) = OpenOptions::new().write(true).open("/dev/kmsg") {
        for line in message.lines() {
            let _ = writeln!(kmsg, "a32-init: {line}");
        }
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(INIT_LOG) {
        let uptime = fs::read_to_string("/proc/uptime").unwrap_or_default();
        let seconds = uptime.split_whitespace().next().unwrap_or("?");
        for line in message.lines() {
            let _ = writeln!(file, "[{seconds}] {line}");
        }
    }
}

fn reap_forever() -> ! {
    loop {
        let mut status = 0;
        // SAFETY: `status` is a valid writable c_int for the call duration.
        let pid = unsafe { waitpid(-1, &mut status, 0) };
        if pid < 0 {
            // No children yet: avoid spinning.
            thread::sleep(Duration::from_secs(1));
        }
    }
}

fn park() -> ! {
    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}
