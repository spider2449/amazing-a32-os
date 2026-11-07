//! The X11 desktop: the X server on the framebuffer with the JWM window manager,
//! both from Alpine. The panel stops drawing while it runs, moves `pointer` for the
//! touches, and draws again once either program ends.

use crate::alpine;
use crate::jwmrc;
use crate::pid1::log;
use std::fs;
use std::io;
use std::os::raw::c_int;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

extern "C" {
    fn kill(pid: c_int, signal: c_int) -> c_int;
}

const SIGKILL: c_int = 9;
const SIGTERM: c_int = 15;

const XORG: &str = "/usr/libexec/Xorg";
const WINDOW_MANAGER: &str = "/usr/bin/jwm";
/// Files the desktop needs in the chroot, and the Alpine package of each.
const NEEDED: &[(&str, &str)] = &[
    (XORG, "xorg-server"),
    ("/usr/lib/xorg/modules/drivers/fbdev_drv.so", "xf86-video-fbdev"),
    ("/usr/lib/xorg/modules/input/evdev_drv.so", "xf86-input-evdev"),
    (WINDOW_MANAGER, "jwm"),
    ("/usr/bin/xterm", "xterm"),
    ("/usr/share/fonts/dejavu", "font-dejavu"),
    ("/usr/bin/svkbd-mobile-intl", "svkbd"),
];
// /run is shared with the chroot, so these paths are the same on both sides.
const CONFIG: &str = "/run/a32-xorg.conf";
const XORG_LOG: &str = "/run/xorg.log";
const SESSION_LOG: &str = "/run/desktop.log";
const DISPLAY: &str = ":0";
const SOCKET: &str = "/data/alpine/tmp/.X11-unix/X0";
const LOCK: &str = "/data/alpine/tmp/.X0-lock";
const SYSTEM_JWMRC: &str = "/data/alpine/etc/system.jwmrc";
const USER_JWMRC: &str = "/data/alpine/root/.jwmrc";
const JWMRC: &str = "/run/a32-jwmrc";

/// The X server took about 4 seconds to listen.
const START_LIMIT: Duration = Duration::from_secs(30);
const STOP_LIMIT: Duration = Duration::from_secs(5);
const CHECK: Duration = Duration::from_millis(250);

/// The running programs, by process id.
pub struct Session {
    xorg: u32,
    window_manager: u32,
}

/// The Alpine packages that are not installed, for the message on the panel.
pub fn missing_packages() -> Vec<&'static str> {
    NEEDED
        .iter()
        .filter(|(path, _)| !Path::new(&format!("{}{path}", alpine::ROOT)).exists())
        .map(|(_, package)| *package)
        .collect()
}

/// Starts the X server on the framebuffer, with `pointer` as its only input, then
/// the window manager once the server listens, and the on-screen keyboard. The
/// size is the screen's, in pixels.
pub fn start(width: u32, height: u32) -> io::Result<Session> {
    let missing = missing_packages();
    if !missing.is_empty() {
        return Err(io::Error::other(format!("not installed: {}", missing.join(" "))));
    }
    if UnixStream::connect(SOCKET).is_ok() {
        return Err(io::Error::other(format!("display {DISPLAY} is already in use")));
    }
    // Alpine's /tmp is on /data, so files of a server from before a restart stay.
    // Nothing listens, so they are stale, and the lock could name a process id that
    // something else has now.
    let _ = fs::remove_file(LOCK);
    let _ = fs::remove_file(SOCKET);
    fs::write(CONFIG, config())?;
    let xorg = alpine::spawn(
        XORG,
        &[DISPLAY, "-config", CONFIG, "-novtswitch", "-sharevts", "-nolisten", "tcp", "-logfile", XORG_LOG],
        &[],
        SESSION_LOG,
    )?;
    let started = Instant::now();
    while UnixStream::connect(SOCKET).is_err() {
        if !running(xorg) {
            return Err(io::Error::other(format!("the X server ended, see {XORG_LOG}")));
        }
        if started.elapsed() > START_LIMIT {
            stop(xorg);
            return Err(io::Error::other(format!("the X server did not start, see {XORG_LOG}")));
        }
        thread::sleep(CHECK);
    }
    let window_manager =
        match alpine::spawn(WINDOW_MANAGER, &window_manager_arguments(height), &[("DISPLAY", DISPLAY)], SESSION_LOG) {
            Ok(window_manager) => window_manager,
            Err(err) => {
                stop(xorg);
                return Err(err);
            }
        };
    // The keyboard shows from the start; the KB button in the tray hides it. It is
    // not watched: closing it leaves the desktop running.
    let keyboard = fs::write(jwmrc::KEYBOARD_SCRIPT, jwmrc::keyboard_script(width, height))
        .and_then(|()| alpine::spawn("/bin/sh", &[jwmrc::KEYBOARD_SCRIPT], &[("DISPLAY", DISPLAY)], SESSION_LOG));
    if let Err(err) = keyboard {
        log(&format!("desktop: the keyboard did not start: {err}"));
    }
    Ok(Session { xorg, window_manager })
}

impl Session {
    /// Waits until either program ends, then stops the other. Returns which one
    /// ended, in words.
    pub fn wait(self) -> &'static str {
        let ended = loop {
            if !running(self.window_manager) {
                break "the window manager ended";
            }
            if !running(self.xorg) {
                break "the X server ended";
            }
            thread::sleep(CHECK);
        };
        stop(self.window_manager);
        stop(self.xorg);
        ended
    }
}

/// JWM's settings: Alpine's own, changed for touch by `jwmrc::for_touch`. A
/// configuration of root's own is used as it is.
fn window_manager_arguments(height: u32) -> Vec<&'static str> {
    if Path::new(USER_JWMRC).exists() {
        return Vec::new();
    }
    let changed = fs::read_to_string(SYSTEM_JWMRC).ok().and_then(|text| jwmrc::for_touch(&text, height));
    match changed {
        Some(text) if fs::write(JWMRC, &text).is_ok() => vec!["-f", JWMRC],
        _ => Vec::new(),
    }
}

/// The X server's settings: the framebuffer, and the pointer instead of the touch
/// screen. No screen saver or power saving, because the init turns the backlight
/// off by itself.
fn config() -> String {
    format!(
        r#"Section "ServerFlags"
    Option "AutoAddDevices" "false"
    Option "AutoEnableDevices" "false"
    Option "DontVTSwitch" "true"
    Option "BlankTime" "0"
    Option "StandbyTime" "0"
    Option "SuspendTime" "0"
    Option "OffTime" "0"
EndSection
Section "Device"
    Identifier "framebuffer"
    Driver "fbdev"
    Option "fbdev" "/dev/fb0"
    Option "ShadowFB" "true"
EndSection
Section "Screen"
    Identifier "screen"
    Device "framebuffer"
EndSection
Section "InputDevice"
    Identifier "pointer"
    Driver "evdev"
    Option "Device" "{device}"
EndSection
Section "ServerLayout"
    Identifier "layout"
    Screen "screen"
    InputDevice "pointer" "CorePointer"
EndSection
"#,
        device = crate::pointer::DEVICE
    )
}

/// Whether the process runs. An ended one stays a zombie until `reap_forever`
/// collects it, so that state counts as ended too.
fn running(pid: u32) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        // The state follows the name, which is in parentheses and may hold spaces.
        Ok(stat) => stat.rsplit_once(") ").is_some_and(|(_, rest)| !rest.starts_with('Z')),
        Err(_) => false,
    }
}

/// Asks the process to end, and ends it if it does not.
fn stop(pid: u32) {
    for signal in [SIGTERM, SIGKILL] {
        if !running(pid) {
            return;
        }
        // SAFETY: kill takes plain integers.
        unsafe { kill(pid as c_int, signal) };
        let asked = Instant::now();
        while running(pid) && asked.elapsed() < STOP_LIMIT {
            thread::sleep(CHECK);
        }
    }
}
