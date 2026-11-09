//! The firewall, and the Alpine Linux userland that runs in a chroot on /data.
//!
//! This process stays PID 1, outside the chroot, and the console does not depend on
//! anything here. So a damaged Alpine cannot take away the way to fastboot.

use crate::pid1::{cstring, log, run_child, stock_command, MS_BIND, MS_REC};
use std::ffi::CString;
use std::fs::{self, File};
use std::io;
use std::os::fd::AsRawFd;
use std::os::raw::{c_char, c_int, c_ulong, c_void};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

pub const ROOT: &str = "/data/alpine";
// A real file: `bin/sh` is a link to an absolute path, which only resolves in the chroot.
const ROOT_MARKER: &str = "/data/alpine/bin/busybox";
const SHELL: &str = "/bin/sh";
const PATH: &str = "/usr/sbin:/usr/bin:/sbin:/bin";
const SHARED_MOUNTS: &[&str] = &["/dev", "/proc", "/sys", "/run"];
const RESOLV_LINK: &str = "/data/alpine/etc/resolv.conf";
const RESOLV_TARGET: &str = "/run/resolv.conf";
const OUTPUT: &str = "/run/alpine-output";

const SSHD: &str = "/usr/sbin/sshd";
const ROOT_SSHD: &str = "/data/alpine/usr/sbin/sshd";
const ROOT_HOST_KEY: &str = "/data/alpine/etc/ssh/ssh_host_ed25519_key";
const SSHD_LOG: &str = "/run/sshd.log";
const SERVICE_CHECK: Duration = Duration::from_secs(5);

const AVAHI: &str = "/usr/sbin/avahi-daemon";
const ROOT_AVAHI: &str = "/data/alpine/usr/sbin/avahi-daemon";
const AVAHI_USER: &str = "avahi";
const ROOT_GROUP: &str = "/data/alpine/etc/group";
const AVAHI_LOG: &str = "/run/avahi.log";
const MDNS_PORT: &str = "5353";

const IPTABLES: &str = "/system/bin/iptables";
const IP6TABLES: &str = "/system/bin/ip6tables";
// SSH and the status page are only for local networks. A router that forwards a port
// does not change the source address, so this also refuses the internet.
const LOCAL_SOURCES: &[&str] = &["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"];
const LOCAL_PORTS: &[&str] = &["22", crate::web::PORT_TEXT];

/// Set when all firewall rules are in place. SSH does not start without them.
static FIREWALL_READY: AtomicBool = AtomicBool::new(false);
/// Whether `sshd` is running.
static SSH_RUNNING: AtomicBool = AtomicBool::new(false);
/// Whether the mounts for the chroot are done.
static PREPARED: Mutex<bool> = Mutex::new(false);

extern "C" {
    fn chroot(path: *const c_char) -> c_int;
    fn setsid() -> c_int;
    fn ioctl(fd: c_int, request: c_int, ...) -> c_int;
    fn chdir(path: *const c_char) -> c_int;
    fn mount(
        source: *const c_char,
        target: *const c_char,
        fstype: *const c_char,
        flags: c_ulong,
        data: *const c_void,
    ) -> c_int;
}

fn table_command(program: &str, arguments: &[&str]) -> io::Result<()> {
    let status = run_child(
        stock_command(program).args(arguments).stdout(Stdio::null()).stderr(Stdio::null()),
    )?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("{program} {arguments:?} ended with {status}")))
    }
}

/// Allows what this system needs, then refuses all other incoming traffic.
/// The policies come last, so that a failed rule cannot cut off the network.
pub fn apply_firewall() -> io::Result<()> {
    let mut rules: Vec<Vec<&str>> = vec![
        vec!["-A", "INPUT", "-i", "lo", "-j", "ACCEPT"],
        vec!["-A", "INPUT", "-m", "state", "--state", "ESTABLISHED,RELATED", "-j", "ACCEPT"],
        vec!["-A", "INPUT", "-p", "icmp", "-j", "ACCEPT"],
        // DHCP replies, which are broadcast and so never look like an open connection.
        vec!["-A", "INPUT", "-p", "udp", "--dport", "68", "-j", "ACCEPT"],
        // DHCP requests from the computer on the USB cable, which has no address yet.
        vec!["-A", "INPUT", "-i", crate::usb::NETWORK_INTERFACE, "-p", "udp", "--dport", "67", "-j", "ACCEPT"],
    ];
    for port in LOCAL_PORTS {
        for source in LOCAL_SOURCES {
            rules.push(vec!["-A", "INPUT", "-p", "tcp", "--dport", port, "-s", source, "-j", "ACCEPT"]);
        }
    }
    // mDNS queries for a32.local. They go to a multicast address, so they never look
    // like replies to the phone.
    for source in LOCAL_SOURCES {
        rules.push(vec!["-A", "INPUT", "-p", "udp", "--dport", MDNS_PORT, "-s", source, "-j", "ACCEPT"]);
    }
    for rule in &rules {
        table_command(IPTABLES, rule)?;
    }
    table_command(IPTABLES, &["-P", "INPUT", "DROP"])?;
    table_command(IPTABLES, &["-P", "FORWARD", "DROP"])?;

    table_command(IP6TABLES, &["-A", "INPUT", "-i", "lo", "-j", "ACCEPT"])?;
    table_command(IP6TABLES, &["-P", "INPUT", "DROP"])?;
    table_command(IP6TABLES, &["-P", "FORWARD", "DROP"])?;

    FIREWALL_READY.store(true, Ordering::Release);
    Ok(())
}

/// Opens a TCP port while `work` runs, for transfers and measurements.
pub fn with_open_port<T>(port: u16, work: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    let port = port.to_string();
    let rule = |action| [action, "INPUT", "-p", "tcp", "--dport", port.as_str(), "-j", "ACCEPT"];
    table_command(IPTABLES, &rule("-I"))?;
    let result = work();
    if let Err(err) = table_command(IPTABLES, &rule("-D")) {
        log(&format!("firewall: could not close a port: {err}"));
    }
    result
}

fn is_installed() -> bool {
    Path::new(ROOT_MARKER).exists()
}

/// Shares the kernel's filesystems with the chroot, once.
fn prepare() -> io::Result<()> {
    let mut prepared = PREPARED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if *prepared {
        return Ok(());
    }
    if !is_installed() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "Alpine is not installed"));
    }
    for shared in SHARED_MOUNTS {
        let target = format!("{ROOT}{shared}");
        fs::create_dir_all(&target)?;
        let source = cstring(shared)?;
        let target = cstring(&target)?;
        // SAFETY: all pointers are valid NUL-terminated strings for the call duration,
        // and a bind mount reads neither the type nor the data.
        let rc = unsafe {
            mount(source.as_ptr(), target.as_ptr(), ptr_none(), MS_BIND | MS_REC, std::ptr::null())
        };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    match std::os::unix::fs::symlink(RESOLV_TARGET, RESOLV_LINK) {
        Err(err) if err.kind() != io::ErrorKind::AlreadyExists => return Err(err),
        _ => {}
    }
    *prepared = true;
    Ok(())
}

const fn ptr_none() -> *const c_char {
    std::ptr::null()
}

/// The `TZ` value for programs in the chroot. Alpine has no zone files here, so
/// without it every clock there shows UTC.
static TIME_ZONE: Mutex<String> = Mutex::new(String::new());

/// Sets the time zone for programs started in the chroot from now on. sshd hands
/// its own `TZ` to SSH sessions, so those follow only after it restarts.
pub fn set_time_zone(tz: String) {
    *TIME_ZONE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = tz;
}

/// A program that runs inside the chroot, with Alpine's environment.
fn chroot_command(program: &str) -> io::Result<Command> {
    let root: CString = cstring(ROOT)?;
    let top: CString = cstring("/")?;
    let mut command = Command::new(program);
    command.env_clear().env("PATH", PATH).env("HOME", "/root").stdin(Stdio::null());
    let tz = TIME_ZONE.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    if !tz.is_empty() {
        command.env("TZ", tz);
    }
    // SAFETY: the closure only makes two system calls, which are async-signal-safe,
    // and it uses strings that were allocated before the fork.
    unsafe {
        command.pre_exec(move || {
            if chroot(root.as_ptr()) != 0 || chdir(top.as_ptr()) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    Ok(command)
}

/// Runs a line with Alpine's shell and returns its output. Like the stock shell
/// command, it writes to a file, so that a background program cannot hold this up.
pub fn run_line(line: &str) -> io::Result<Vec<u8>> {
    prepare()?;
    let output = File::create(OUTPUT)?;
    let status = run_child(
        chroot_command(SHELL)?
            .args(["-c", line])
            .stdout(output.try_clone()?)
            .stderr(output),
    )?;
    let mut text = fs::read(OUTPUT)?;
    if !status.success() {
        text.extend(format!("[{status}]\n").into_bytes());
    }
    Ok(text)
}

// Pseudo-terminal requests, as numbered on ARM Linux.
const TIOCSPTLCK: c_int = 0x4004_5431;
const TIOCGPTN: c_int = 0x8004_5430u32 as c_int;
const TIOCSWINSZ: c_int = 0x5414;
const TIOCSCTTY: c_int = 0x540E;
const O_NOCTTY: i32 = 0o400;

/// Starts Alpine's login shell on a new pseudo-terminal of this size, for the
/// panel's console. Returns the terminal's master side, which carries what the
/// shell writes and takes what is typed, and the shell process.
pub fn start_shell(columns: usize, rows: usize) -> io::Result<(File, std::process::Child)> {
    use std::os::unix::fs::OpenOptionsExt;
    prepare()?;
    let master = File::options().read(true).write(true).custom_flags(O_NOCTTY).open("/dev/ptmx")?;
    let mut unlock: c_int = 0;
    let mut number: u32 = 0;
    // SAFETY: both requests take a pointer to an int, valid for the call.
    unsafe {
        if ioctl(master.as_raw_fd(), TIOCSPTLCK, &mut unlock as *mut c_int) != 0
            || ioctl(master.as_raw_fd(), TIOCGPTN, &mut number as *mut u32) != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    set_terminal_size(&master, columns, rows)?;
    let follower = File::options().read(true).write(true).custom_flags(O_NOCTTY).open(format!("/dev/pts/{number}"))?;
    let mut command = chroot_command(SHELL)?;
    command
        .arg("-l")
        .env("TERM", "vt100")
        .env("SHELL", SHELL)
        .env("USER", "root")
        .stdin(follower.try_clone()?)
        .stdout(follower.try_clone()?)
        .stderr(follower);
    // SAFETY: only async-signal-safe system calls, after the standard streams are
    // in place: a new session, with the terminal on stdin as its controlling one.
    unsafe {
        command.pre_exec(|| {
            if setsid() < 0 || ioctl(0, TIOCSCTTY, 0 as c_int) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command.spawn()?;
    Ok((master, child))
}

/// Tells the terminal, and so the programs on it, how big it is.
pub fn set_terminal_size(master: &File, columns: usize, rows: usize) -> io::Result<()> {
    // struct winsize: rows, columns, then the size in pixels, unused here.
    let mut size: [u16; 4] = [rows as u16, columns as u16, 0, 0];
    // SAFETY: the request reads a struct winsize, which `size` matches.
    if unsafe { ioctl(master.as_raw_fd(), TIOCSWINSZ, size.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn ssh_running() -> bool {
    SSH_RUNNING.load(Ordering::Acquire)
}

/// Keeps `sshd` running once Alpine has it, so that installing needs no restart.
pub fn ssh_service() {
    keep_running(
        "ssh: sshd",
        SSHD_LOG,
        || Path::new(ROOT_SSHD).exists() && Path::new(ROOT_HOST_KEY).exists(),
        start_sshd,
        |running| SSH_RUNNING.store(running, Ordering::Release),
    );
}

/// Keeps `avahi-daemon` running once Alpine has it, so that `a32.local` finds the
/// phone at whatever address DHCP gave it.
pub fn mdns_service() {
    keep_running("mdns: avahi-daemon", AVAHI_LOG, || Path::new(ROOT_AVAHI).exists(), start_avahi, |_| {});
}

/// Starts a program once the firewall is ready and `installed` says Alpine has it,
/// and starts it again whenever it ends.
fn keep_running(
    name: &str,
    log_path: &str,
    installed: impl Fn() -> bool,
    start: impl Fn() -> io::Result<u32>,
    set_running: impl Fn(bool),
) {
    let mut reported = false;
    loop {
        if FIREWALL_READY.load(Ordering::Acquire) && installed() {
            match start() {
                Ok(process) => {
                    log(&format!("{name} started as {process}"));
                    set_running(true);
                    // `reap_forever` collects it, so its entry in /proc shows it ended.
                    let entry = format!("/proc/{process}");
                    while Path::new(&entry).exists() {
                        thread::sleep(SERVICE_CHECK);
                    }
                    set_running(false);
                    log(&format!("{name} ended, see {log_path}"));
                    reported = false;
                }
                Err(err) if !reported => {
                    log(&format!("{name}: {err}"));
                    reported = true;
                }
                Err(_) => {}
            }
        }
        thread::sleep(SERVICE_CHECK);
    }
}

/// Whether the chroot has this program: an absolute path as it is, or a name
/// found on the chroot's PATH.
pub fn has_program(name: &str) -> bool {
    if name.starts_with('/') {
        return Path::new(&format!("{ROOT}{name}")).is_file();
    }
    PATH.split(':').any(|dir| Path::new(&format!("{ROOT}{dir}/{name}")).is_file())
}

/// Starts a program in the chroot without waiting for it, with its output appended
/// to `log_path`, a path outside the chroot. Returns its process id; `reap_forever`
/// collects it when it ends, so its entry in /proc shows whether it still runs.
pub fn spawn(program: &str, arguments: &[&str], environment: &[(&str, &str)], log_path: &str) -> io::Result<u32> {
    prepare()?;
    let log_file = File::options().create(true).append(true).open(log_path)?;
    let child = chroot_command(program)?
        .args(arguments)
        .envs(environment.iter().copied())
        .stdout(log_file.try_clone()?)
        .stderr(log_file)
        .spawn()?;
    Ok(child.id())
}

fn start_avahi() -> io::Result<u32> {
    let group = fs::read_to_string(ROOT_GROUP)?;
    if let Some(updated) = crate::mdns::with_network_member(&group, AVAHI_USER) {
        fs::write(ROOT_GROUP, updated)?;
        log("mdns: added avahi to the network group");
    }
    fs::write(crate::mdns::CONFIG_PATH, crate::mdns::CONFIG)?;
    // In the foreground, so that the process to watch is the daemon itself.
    spawn(AVAHI, &["-f", crate::mdns::CONFIG_PATH], &[], AVAHI_LOG)
}

fn start_sshd() -> io::Result<u32> {
    prepare()?;
    let log_file = File::options().create(true).append(true).open(SSHD_LOG)?;
    let child = chroot_command(SSHD)?
        .args(["-D", "-e"])
        .stdout(log_file.try_clone()?)
        .stderr(log_file)
        .spawn()?;
    Ok(child.id())
}
