//! PID 1 duties: filesystems, device nodes, logging, USB console, status screen
//! and child reaping.
//!
//! The stock 3.18 kernel has no devtmpfs, so `/dev` is a tmpfs populated by hand.
//! PID 1 must never exit, because that panics the kernel and reboots the phone.
//!
//! The phone can only reach fastboot from software, because no key combination
//! does it. So if this process panics or the console cannot start, it reboots to
//! fastboot, where the system can be repaired. A kernel panic is not covered.

use crate::display::{status_lines, Canvas};
use crate::fb::Framebuffer;
use crate::shell::{self, Platform};
use crate::usb;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Write};
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

extern "C" {
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
const EXPOSED_PARTITIONS: &[&str] = &["boot", "recovery"];
const SERIAL_WAIT_SECONDS: u32 = 30;
// Must exceed SERIAL_WAIT_SECONDS, so that this only catches a stuck setup.
const CONSOLE_DEADLINE: Duration = Duration::from_secs(60);
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

    for error in &errors {
        log(error);
    }
    log(&crate::probe::report());

    if let Err(err) = thread::Builder::new().spawn(serial_console) {
        fail_safe(&format!("console thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(console_deadline) {
        log(&format!("console deadline thread failed: {err}"));
    }
    if let Err(err) = thread::Builder::new().spawn(status_screen) {
        log(&format!("display thread failed: {err}"));
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

    let mut canvas = Canvas::new(format);
    loop {
        let uptime = fs::read_to_string("/proc/uptime")?;
        let capacity = fs::read_to_string(BATTERY_CAPACITY).ok();
        let status = fs::read_to_string(BATTERY_STATUS).ok();
        canvas.render_status(&status_lines(&uptime, capacity.as_deref(), status.as_deref()));
        framebuffer.present(canvas.pixels())?;
        thread::sleep(Duration::from_secs(1));
    }
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

    fn power_off(&self) -> io::Error {
        // SAFETY: `sync` takes no arguments and has no memory safety requirements.
        unsafe { sync() };
        // SAFETY: this reboot command takes no pointer arguments.
        unsafe { syscall(SYS_REBOOT, REBOOT_MAGIC1, REBOOT_MAGIC2, REBOOT_CMD_POWER_OFF, 0) };
        io::Error::last_os_error()
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
