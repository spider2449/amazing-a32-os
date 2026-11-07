//! A mouse made with `uinput`, for the desktop. The touch screen reports the old
//! multi-touch protocol A, which the X server's evdev driver does not understand,
//! so the init reads the touches and moves this simpler device instead.

use crate::input::{PointerButton, PointerEvent, EVENT_LEN};
use crate::pid1::make_char_device;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::os::raw::c_int;
use std::path::Path;
use std::thread;
use std::time::Duration;

extern "C" {
    fn ioctl(fd: c_int, request: c_int, ...) -> c_int;
}

/// The name the kernel lists, and the node the X server is told to open.
pub const NAME: &str = "a32-pointer";
pub const DEVICE: &str = "/dev/input/a32-pointer";

const UINPUT_SYSFS_DEV: &str = "/sys/class/misc/uinput/dev";
const UINPUT_DEVICE: &str = "/dev/uinput";

// Requests on ARM Linux: _IOW('U', 100, int) and the next ones, and _IO('U', 1).
const UI_SET_EVBIT: c_int = 0x4004_5564;
const UI_SET_KEYBIT: c_int = 0x4004_5565;
const UI_SET_ABSBIT: c_int = 0x4004_5567;
const UI_DEV_CREATE: c_int = 0x5501;

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const SYN_REPORT: u16 = 0;
const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const ABS_X: u16 = 0;
const ABS_Y: u16 = 1;
const ABS_COUNT: usize = 64;
const NAME_LEN: usize = 80;
const BUS_VIRTUAL: u16 = 6;

pub struct Pointer {
    file: File,
}

impl Pointer {
    /// Makes the device, with axes as large as the screen, and its node in /dev.
    pub fn create(width: u32, height: u32) -> io::Result<Self> {
        let dev = fs::read_to_string(UINPUT_SYSFS_DEV)?;
        let (major, minor) = crate::usb::parse_dev(&dev)
            .ok_or_else(|| io::Error::other(format!("bad device number: {dev}")))?;
        if !Path::new(UINPUT_DEVICE).exists() {
            make_char_device(UINPUT_DEVICE, major, minor, 0o600)?;
        }
        let file = File::options().write(true).open(UINPUT_DEVICE)?;
        let fd = file.as_raw_fd();
        let set = |request: c_int, value: u16| {
            // SAFETY: these requests take an integer argument, not a pointer.
            if unsafe { ioctl(fd, request, c_int::from(value)) } != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        };
        set(UI_SET_EVBIT, EV_KEY)?;
        set(UI_SET_EVBIT, EV_ABS)?;
        set(UI_SET_EVBIT, EV_SYN)?;
        set(UI_SET_KEYBIT, BTN_LEFT)?;
        set(UI_SET_KEYBIT, BTN_RIGHT)?;
        set(UI_SET_ABSBIT, ABS_X)?;
        set(UI_SET_ABSBIT, ABS_Y)?;
        (&file).write_all(&user_dev(width, height))?;
        // SAFETY: UI_DEV_CREATE takes no argument.
        if unsafe { ioctl(fd, UI_DEV_CREATE) } != 0 {
            return Err(io::Error::last_os_error());
        }
        make_node()?;
        Ok(Self { file })
    }

    /// Sends the events, each as its own report, so that a move lands before a
    /// press at the new place.
    pub fn send(&mut self, events: &[PointerEvent]) -> io::Result<()> {
        let mut bytes = Vec::with_capacity(events.len() * 3 * EVENT_LEN);
        for event in events {
            match *event {
                PointerEvent::Move(x, y) => {
                    bytes.extend(raw(EV_ABS, ABS_X, x));
                    bytes.extend(raw(EV_ABS, ABS_Y, y));
                }
                PointerEvent::Press(button) => bytes.extend(raw(EV_KEY, code(button), 1)),
                PointerEvent::Release(button) => bytes.extend(raw(EV_KEY, code(button), 0)),
            }
            bytes.extend(raw(EV_SYN, SYN_REPORT, 0));
        }
        self.file.write_all(&bytes)
    }
}

fn code(button: PointerButton) -> u16 {
    match button {
        PointerButton::Left => BTN_LEFT,
        PointerButton::Right => BTN_RIGHT,
    }
}

/// `struct input_event` on 32-bit ARM. The kernel fills in the time.
fn raw(kind: u16, code: u16, value: i32) -> [u8; EVENT_LEN] {
    let mut event = [0u8; EVENT_LEN];
    event[8..10].copy_from_slice(&kind.to_ne_bytes());
    event[10..12].copy_from_slice(&code.to_ne_bytes());
    event[12..16].copy_from_slice(&value.to_ne_bytes());
    event
}

/// `struct uinput_user_dev`: the name, the bus and ids, the number of force
/// feedback effects, then the maximum, minimum, fuzz and flat of every axis.
fn user_dev(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = vec![0u8; NAME_LEN + 8 + 4 + 4 * ABS_COUNT * 4];
    bytes[..NAME.len()].copy_from_slice(NAME.as_bytes());
    bytes[NAME_LEN..NAME_LEN + 2].copy_from_slice(&BUS_VIRTUAL.to_ne_bytes());
    let maximum = NAME_LEN + 8 + 4;
    let mut set_max = |axis: u16, value: u32| {
        let at = maximum + usize::from(axis) * 4;
        bytes[at..at + 4].copy_from_slice(&(value as i32).to_ne_bytes());
    };
    set_max(ABS_X, width - 1);
    set_max(ABS_Y, height - 1);
    bytes
}

/// Makes the node for the new device, whose number the kernel only gives once the
/// device exists. An old node is replaced, because the number can differ.
fn make_node() -> io::Result<()> {
    fs::create_dir_all("/dev/input")?;
    let mut last = io::Error::other(format!("{NAME} not found"));
    for _ in 0..20 {
        match crate::pid1::find_input_device(NAME) {
            Ok((major, minor)) => {
                let _ = fs::remove_file(DEVICE);
                return make_char_device(DEVICE, major, minor, 0o600);
            }
            Err(err) => last = err,
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(last)
}
