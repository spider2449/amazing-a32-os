//! Kernel capability report for userspace compatibility decisions.
//!
//! The stock Amazing A32 kernel is old and vendor-modified, so version checks are
//! not enough: features may be missing or backported. This module inspects the
//! running kernel and, on ARM Linux, probes syscall numbers directly. A syscall is
//! considered absent only when the kernel returns ENOSYS.

use std::fs;
use std::path::Path;

#[cfg(all(target_os = "linux", target_arch = "arm"))]
use std::os::raw::c_long;

const NAMESPACES: &[(&str, &str)] = &[
    ("mount", "mnt"),
    ("user", "user"),
    ("pid", "pid"),
    ("network", "net"),
];

const CGROUPS: &[&str] = &["cpu", "cpuacct", "freezer", "memory", "pids"];
const FILESYSTEMS: &[&str] = &["ext4", "fuse", "overlay", "squashfs", "f2fs"];
const DEVICES: &[(&str, &str)] = &[
    ("tun", "/sys/class/misc/tun"),
    ("loop-control", "/sys/class/misc/loop-control"),
];

// ARM EABI syscall numbers from Linux arch/arm/tools/syscall.tbl.
const SYSCALLS: &[(&str, i64)] = &[
    ("getrandom", 384),
    ("memfd_create", 385),
    ("copy_file_range", 391),
    ("statx", 397),
    ("io_uring_setup", 425),
    ("pidfd_open", 434),
    ("clone3", 435),
    ("openat2", 437),
];

pub fn report() -> String {
    let mut out = String::from("A32 kernel compatibility\n");

    out.push_str("\nkernel:\n");
    let release = read_trim("/proc/sys/kernel/osrelease")
        .or_else(|| read_trim("/proc/version"))
        .unwrap_or_else(|| "unknown".into());
    out.push_str(&format!("  release              {release}\n"));

    out.push_str("\nnamespaces:\n");
    for (label, name) in NAMESPACES {
        let present = Path::new("/proc/self/ns").join(name).exists();
        out.push_str(&format!("  {label:<20} {}\n", yes_no(present)));
    }

    out.push_str("\ncgroups:\n");
    let cgroups = fs::read_to_string("/proc/cgroups").unwrap_or_default();
    for name in CGROUPS {
        out.push_str(&format!("  {name:<20} {}\n", yes_no(cgroup_enabled(&cgroups, name))));
    }

    out.push_str("\nfilesystems:\n");
    let filesystems = fs::read_to_string("/proc/filesystems").unwrap_or_default();
    for name in FILESYSTEMS {
        out.push_str(&format!("  {name:<20} {}\n", yes_no(filesystem_present(&filesystems, name))));
    }

    out.push_str("\nregistered devices:\n");
    for (name, path) in DEVICES {
        out.push_str(&format!("  {name:<20} {}\n", yes_no(Path::new(path).exists())));
    }

    out.push_str("\nmodern syscalls:\n");
    for (name, number) in SYSCALLS {
        let state = match syscall_present(*number) {
            Some(true) => "yes",
            Some(false) => "no (ENOSYS)",
            None => "not probed on this build",
        };
        out.push_str(&format!("  {name:<20} {state}\n"));
    }

    out
}

fn read_trim(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn cgroup_enabled(text: &str, name: &str) -> bool {
    text.lines().any(|line| {
        if line.starts_with('#') {
            return false;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        fields.len() >= 4 && fields[0] == name && fields[3] == "1"
    })
}

fn filesystem_present(text: &str, name: &str) -> bool {
    text.lines().any(|line| line.split_whitespace().last() == Some(name))
}

#[cfg(all(target_os = "linux", target_arch = "arm"))]
fn syscall_present(number: i64) -> Option<bool> {
    const ENOSYS: i32 = 38;

    extern "C" {
        fn syscall(number: c_long, ...) -> c_long;
    }

    // SAFETY: all probes pass six zero machine words. The Linux syscall entry
    // ignores arguments beyond the syscall's arity. We only distinguish ENOSYS
    // from every other result; EINVAL, EFAULT, EBADF, EPERM or success all prove
    // that the syscall number is implemented by this kernel.
    let result = unsafe { syscall(number as c_long, 0 as c_long, 0 as c_long, 0 as c_long, 0 as c_long, 0 as c_long, 0 as c_long) };
    if result == -1 {
        return Some(std::io::Error::last_os_error().raw_os_error() != Some(ENOSYS));
    }
    Some(true)
}

#[cfg(not(all(target_os = "linux", target_arch = "arm")))]
fn syscall_present(_number: i64) -> Option<bool> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_enabled_cgroups() {
        let text = "#subsys_name hierarchy num_cgroups enabled\ncpu 2 1 1\nmemory 0 1 0\n";
        assert!(cgroup_enabled(text, "cpu"));
        assert!(!cgroup_enabled(text, "memory"));
        assert!(!cgroup_enabled(text, "pids"));
    }

    #[test]
    fn parses_filesystems() {
        let text = "nodev\tsysfs\n\text4\nnodev\tfuse\n";
        assert!(filesystem_present(text, "ext4"));
        assert!(filesystem_present(text, "fuse"));
        assert!(!filesystem_present(text, "overlay"));
    }

    #[test]
    fn report_is_available_on_host_tests() {
        assert!(report().starts_with("A32 kernel compatibility\n"));
    }
}
