//! PID 1 duties: filesystems, device nodes, logging, USB console, child reaping.
//!
//! The stock 3.18 kernel has no devtmpfs, so `/dev` is a tmpfs populated by hand.
//! PID 1 must never exit, because that panics the kernel and reboots the phone.

use crate::shell::{self, Platform};
use crate::usb;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, Write};
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void};
use std::path::Path;
use std::ptr;
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
const SERIAL_DEVICE: &str = "/dev/ttyGS0";
const SERIAL_WAIT_SECONDS: u32 = 30;

const SYSLOG_ACTION_READ_ALL: c_int = 3;
const SYSLOG_ACTION_SIZE_BUFFER: c_int = 10;

// The reboot syscall number on 32-bit ARM EABI, the only supported target.
const SYS_REBOOT: c_long = 88;
const REBOOT_MAGIC1: c_ulong = 0xfee1_dead;
const REBOOT_MAGIC2: c_ulong = 0x2812_1969;
const REBOOT_CMD_RESTART: c_ulong = 0x0123_4567;
const REBOOT_CMD_RESTART2: c_ulong = 0xa1b2_c3d4;

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
    // A panic hook that parks forever stops panic=abort from killing PID 1.
    std::panic::set_hook(Box::new(|info| {
        log(&format!("panic: {info}"));
        park();
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

    for error in &errors {
        log(error);
    }
    log(&crate::probe::report());

    if let Err(err) = thread::Builder::new().spawn(serial_console) {
        log(&format!("console thread failed: {err}"));
    }

    log("boot complete");
    reap_forever()
}

/// Brings up the USB serial port and serves the shell on it forever.
fn serial_console() {
    if let Err(err) = usb::enable_serial(Path::new(usb::ANDROID_USB)) {
        log(&format!("usb gadget failed: {err}"));
        return;
    }
    let Some((major, minor)) = wait_for_serial_device() else {
        log("usb serial device did not appear");
        return;
    };
    if let Err(err) = make_char_device(SERIAL_DEVICE, major, minor, 0o600) {
        log(&format!("mknod {SERIAL_DEVICE} failed: {err}"));
        return;
    }
    log("usb serial console ready");

    // Open and read fail while no host is attached, and input ends on
    // disconnect, so keep retrying for the lifetime of the system.
    loop {
        if let Err(err) = serve_shell() {
            log(&format!("console session ended: {err}"));
        }
        thread::sleep(Duration::from_secs(1));
    }
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

fn make_char_device(path: &str, major: u32, minor: u32, perm: c_uint) -> io::Result<()> {
    let path = cstring(path)?;
    // SAFETY: `path` is a valid NUL-terminated string for the call duration.
    let rc = unsafe { mknod(path.as_ptr(), S_IFCHR | perm, makedev(major, minor)) };
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
