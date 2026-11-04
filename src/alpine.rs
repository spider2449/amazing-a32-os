//! The firewall, and the Alpine Linux userland that runs in a chroot on /data.
//!
//! This process stays PID 1, outside the chroot, and the console does not depend on
//! anything here. So a damaged Alpine cannot take away the way to fastboot.

use crate::pid1::{cstring, log, run_child, stock_command, MS_BIND, MS_REC};
use std::ffi::CString;
use std::fs::{self, File};
use std::io;
use std::os::raw::{c_char, c_int, c_ulong, c_void};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

const ROOT: &str = "/data/alpine";
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
const SSHD_CHECK: Duration = Duration::from_secs(5);

const IPTABLES: &str = "/system/bin/iptables";
const IP6TABLES: &str = "/system/bin/ip6tables";
// SSH is only for local networks. A router that forwards a port does not change
// the source address, so this also refuses the internet.
const SSH_SOURCES: &[&str] = &["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"];

/// Set when all firewall rules are in place. SSH does not start without them.
static FIREWALL_READY: AtomicBool = AtomicBool::new(false);
/// Whether `sshd` is running.
static SSH_RUNNING: AtomicBool = AtomicBool::new(false);
/// Whether the mounts for the chroot are done.
static PREPARED: Mutex<bool> = Mutex::new(false);

extern "C" {
    fn chroot(path: *const c_char) -> c_int;
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
    ];
    for source in SSH_SOURCES {
        rules.push(vec!["-A", "INPUT", "-p", "tcp", "--dport", "22", "-s", source, "-j", "ACCEPT"]);
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

/// A program that runs inside the chroot, with Alpine's environment.
fn chroot_command(program: &str) -> io::Result<Command> {
    let root: CString = cstring(ROOT)?;
    let top: CString = cstring("/")?;
    let mut command = Command::new(program);
    command.env_clear().env("PATH", PATH).env("HOME", "/root").stdin(Stdio::null());
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

pub fn ssh_running() -> bool {
    SSH_RUNNING.load(Ordering::Acquire)
}

/// Keeps `sshd` running once Alpine has it, so that installing needs no restart.
pub fn ssh_service() {
    let mut reported = false;
    loop {
        let ready = FIREWALL_READY.load(Ordering::Acquire)
            && Path::new(ROOT_SSHD).exists()
            && Path::new(ROOT_HOST_KEY).exists();
        if ready {
            match start_sshd() {
                Ok(process) => {
                    log(&format!("ssh: sshd started as {process}"));
                    SSH_RUNNING.store(true, Ordering::Release);
                    // `reap_forever` collects it, so its entry in /proc shows it ended.
                    let entry = format!("/proc/{process}");
                    while Path::new(&entry).exists() {
                        thread::sleep(SSHD_CHECK);
                    }
                    SSH_RUNNING.store(false, Ordering::Release);
                    log(&format!("ssh: sshd ended, see {SSHD_LOG}"));
                    reported = false;
                }
                Err(err) if !reported => {
                    log(&format!("ssh: {err}"));
                    reported = true;
                }
                Err(_) => {}
            }
        }
        thread::sleep(SSHD_CHECK);
    }
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
