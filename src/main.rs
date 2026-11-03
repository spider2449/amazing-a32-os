//! Userspace entry point for the Amazing A32.
//!
//! Running as PID 1 boots the system. Running as any other PID only prints a
//! read-only device report, so the binary is safe to try under stock Android.

#[cfg(any(target_os = "linux", test))]
mod bench;
mod probe;
#[cfg(any(target_os = "linux", test))]
mod properties;

#[cfg(any(target_os = "linux", test))]
mod dhcp;
#[cfg(any(target_os = "linux", test))]
mod display;
#[cfg(any(target_os = "linux", test))]
mod dump;
#[cfg(target_os = "linux")]
mod fb;
#[cfg(any(target_os = "linux", test))]
mod input;
#[cfg(target_os = "linux")]
mod pid1;
#[cfg(any(target_os = "linux", test))]
mod shell;
#[cfg(any(target_os = "linux", test))]
mod sntp;
#[cfg(any(target_os = "linux", test))]
mod thermal;
#[cfg(any(target_os = "linux", test))]
mod usb;
#[cfg(any(target_os = "linux", test))]
mod wifi;

fn main() {
    #[cfg(target_os = "linux")]
    if std::process::id() == 1 {
        pid1::run();
    }

    print!("{}", probe::report());
}
