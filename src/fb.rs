//! Linux framebuffer device access.

use crate::display::{Format, FIX_LEN, VAR_LEN};
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::raw::{c_int, c_void};
use std::ptr;

extern "C" {
    fn ioctl(fd: c_int, request: c_int, ...) -> c_int;
    fn mmap(
        address: *mut c_void,
        length: usize,
        protection: c_int,
        flags: c_int,
        fd: c_int,
        // musl's off_t has 64 bits, also on 32-bit ARM.
        offset: i64,
    ) -> *mut c_void;
    fn munmap(address: *mut c_void, length: usize) -> c_int;
}

const PROT_READ: c_int = 1;
const PROT_WRITE: c_int = 2;
const MAP_SHARED: c_int = 1;

const FBIOGET_VSCREENINFO: c_int = 0x4600;
const FBIOGET_FSCREENINFO: c_int = 0x4602;
const FBIOPAN_DISPLAY: c_int = 0x4606;
const FBIOBLANK: c_int = 0x4611;
const FB_BLANK_UNBLANK: c_int = 0;

// Byte offsets of `xoffset` and `yoffset` in `fb_var_screeninfo`.
const VAR_XOFFSET: usize = 16;
const VAR_YOFFSET: usize = 20;

/// The screen's memory, mapped into this process. Copying to it was about 100 times
/// faster than `write` on this phone: a whole frame took 153 ms that way.
pub struct Framebuffer {
    file: File,
    var: [u8; VAR_LEN],
    format: Format,
    mapping: *mut u8,
    mapping_len: usize,
    /// What the screen shows, so that only rows that changed are copied.
    shown: Vec<u8>,
}

// SAFETY: the mapping belongs to this value, and is only reached through `&mut self`.
unsafe impl Send for Framebuffer {}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        // SAFETY: the address and length come from a successful `mmap`, and nothing
        // can use the mapping after this.
        unsafe { munmap(self.mapping.cast(), self.mapping_len) };
    }
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

        let frame_len = (format.stride * format.height) as usize;
        // SAFETY: a new mapping chosen by the kernel touches no existing memory. The
        // result is checked before use.
        let mapping = unsafe {
            mmap(
                ptr::null_mut(),
                frame_len,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                file.as_raw_fd(),
                0,
            )
        };
        // MAP_FAILED is -1.
        if mapping as isize == -1 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            file,
            var,
            format,
            mapping: mapping.cast(),
            mapping_len: frame_len,
            shown: Vec::new(),
        })
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

    /// Shows a frame. It does nothing at all if no row changed, because telling the
    /// driver to update the panel costs time too.
    pub fn present(&mut self, pixels: &[u8]) -> io::Result<()> {
        if self.copy_changes(pixels)? > 0 {
            self.pan()?;
        }
        Ok(())
    }

    /// Copies the rows that differ from what the screen shows. Returns how many.
    pub fn copy_changes(&mut self, pixels: &[u8]) -> io::Result<usize> {
        if pixels.len() != self.mapping_len {
            return Err(io::Error::other("wrong frame size"));
        }
        if self.shown.is_empty() {
            // The screen's contents are not known yet, so this copies everything.
            self.shown = pixels.iter().map(|byte| !byte).collect();
        }
        let stride = self.format.stride as usize;
        let mut changed = 0;
        for (index, (new, old)) in
            pixels.chunks_exact(stride).zip(self.shown.chunks_exact_mut(stride)).enumerate()
        {
            if new != old {
                old.copy_from_slice(new);
                // SAFETY: the row lies inside the mapping, because `pixels` has the
                // mapping's length and rows are cut from it, and the regions are separate.
                unsafe {
                    ptr::copy_nonoverlapping(new.as_ptr(), self.mapping.add(index * stride), stride);
                }
                changed += 1;
            }
        }
        Ok(changed)
    }

    /// Asks the driver to show what was copied.
    pub fn pan(&mut self) -> io::Result<()> {
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
