//! Linux framebuffer device access.

use crate::display::{Format, FIX_LEN, VAR_LEN};
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::raw::{c_int, c_void};
use std::os::unix::fs::FileExt;

extern "C" {
    fn ioctl(fd: c_int, request: c_int, ...) -> c_int;
}

const FBIOGET_VSCREENINFO: c_int = 0x4600;
const FBIOGET_FSCREENINFO: c_int = 0x4602;
const FBIOPAN_DISPLAY: c_int = 0x4606;
const FBIOBLANK: c_int = 0x4611;
const FB_BLANK_UNBLANK: c_int = 0;

// Byte offsets of `xoffset` and `yoffset` in `fb_var_screeninfo`.
const VAR_XOFFSET: usize = 16;
const VAR_YOFFSET: usize = 20;

pub struct Framebuffer {
    file: File,
    var: [u8; VAR_LEN],
    format: Format,
}

impl Framebuffer {
    pub fn open(path: &str) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let mut var = [0u8; VAR_LEN];
        let mut fix = [0u8; FIX_LEN];
        query(&file, FBIOGET_VSCREENINFO, var.as_mut_ptr().cast())?;
        query(&file, FBIOGET_FSCREENINFO, fix.as_mut_ptr().cast())?;
        let format = Format::from_screeninfo(&var, &fix).map_err(io::Error::other)?;

        // Show the first buffer page, which `present` writes to.
        var[VAR_XOFFSET..VAR_YOFFSET + 4].fill(0);
        Ok(Self { file, var, format })
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// Requests that the panel turns on. Drivers may refuse if it already is.
    pub fn unblank(&self) -> io::Result<()> {
        // SAFETY: FBIOBLANK takes an integer argument, not a pointer.
        let rc = unsafe { ioctl(self.file.as_raw_fd(), FBIOBLANK, FB_BLANK_UNBLANK) };
        check(rc)
    }

    /// Copies a frame to the first buffer page and asks the driver to show it.
    pub fn present(&mut self, pixels: &[u8]) -> io::Result<()> {
        self.file.write_all_at(pixels, 0)?;
        query(&self.file, FBIOPAN_DISPLAY, self.var.as_mut_ptr().cast())
    }
}

fn query(file: &File, request: c_int, argument: *mut c_void) -> io::Result<()> {
    // SAFETY: callers pass a buffer of the size the given request reads or
    // writes, and it stays valid for the duration of the call.
    let rc = unsafe { ioctl(file.as_raw_fd(), request, argument) };
    check(rc)
}

fn check(rc: c_int) -> io::Result<()> {
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
