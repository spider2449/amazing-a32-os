//! Userspace entry point for the Amazing A32.
//!
//! Running as PID 1 boots the system. Running as any other PID only prints a
//! read-only device report, so the binary is safe to try under stock Android.

#[cfg(target_os = "linux")]
mod alpine;
#[cfg(target_os = "linux")]
mod camera;
#[cfg(target_os = "linux")]
mod camera_regs;
#[cfg(any(target_os = "linux", test))]
mod bench;
#[cfg(any(target_os = "linux", test))]
mod bluetooth;
#[cfg(any(target_os = "linux", test))]
mod identify;
#[cfg(any(target_os = "linux", test))]
mod charging;
mod probe;
#[cfg(any(target_os = "linux", test))]
mod photo;
#[cfg(any(target_os = "linux", test))]
mod properties;

#[cfg(any(target_os = "linux", test))]
mod audio;
#[cfg(target_os = "linux")]
mod camera_window;
#[cfg(target_os = "linux")]
mod desktop;
#[cfg(any(target_os = "linux", test))]
mod jwmrc;
#[cfg(target_os = "linux")]
mod pointer;
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
mod sensors;
#[cfg(any(target_os = "linux", test))]
mod settings;
#[cfg(any(target_os = "linux", test))]
mod shell;
#[cfg(any(target_os = "linux", test))]
mod mdns;
#[cfg(any(target_os = "linux", test))]
mod sntp;
#[cfg(any(target_os = "linux", test))]
mod swap;
#[cfg(any(target_os = "linux", test))]
mod thermal;
#[cfg(any(target_os = "linux", test))]
mod transfer;
#[cfg(any(target_os = "linux", test))]
mod terminal;
mod ui;
#[cfg(any(target_os = "linux", test))]
mod usb;
#[cfg(any(target_os = "linux", test))]
mod watchdog;
#[cfg(any(target_os = "linux", test))]
mod web;
#[cfg(any(target_os = "linux", test))]
mod wifi;
#[cfg(any(target_os = "linux", test))]
mod x11;

fn main() {
    #[cfg(target_os = "linux")]
    if std::process::id() == 1 {
        pid1::run();
    }

    print!("{}", probe::report());
}
