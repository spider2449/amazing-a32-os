//! Takes photos with the rear (GC5025) and front (GC2385) cameras.
//!
//! There is no camera HAL here: the kernel's sensor driver powers and programs
//! the sensor, and this code sets up the sensor interface (seninf), the MIPI
//! receiver and the ISP's pass 1 the way the stock HAL does, then lets the ISP's
//! IMGO DMA write raw frames into an ION buffer. The seninf steps copy the stock
//! `libcam.halsensor.so` (SeninfDrvImp, HalSensor::seninfControl); the ISP values
//! were read from the hardware while stock Android previewed (camera_regs.rs).
//! Auto exposure adjusts shutter and gain from frame to frame, and photo.rs
//! turns the last frame into a JPEG. One photo at a time.

use crate::camera_regs;
use crate::photo::{self, Exposure, ExposureLimits, Rotation, UNIT_GAIN};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::raw::{c_int, c_ulong, c_void};
use std::ptr;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

extern "C" {
    fn ioctl(fd: c_int, request: c_int, ...) -> c_int;
    fn mmap(address: *mut c_void, length: usize, protection: c_int, flags: c_int, fd: c_int, offset: i64)
        -> *mut c_void;
    fn munmap(address: *mut c_void, length: usize) -> c_int;
}

const PROT_READ: c_int = 1;
const PROT_WRITE: c_int = 2;
const MAP_SHARED: c_int = 1;

const SENSOR_DEVICE: &str = "/dev/kd_camera_hw";
const ISP_DEVICE: &str = "/dev/camera-isp";
const ION_DEVICE: &str = "/dev/ion";
/// Where the kernel publishes each device's number.
const DEVICE_NUMBERS: [(&str, &str); 3] = [
    (SENSOR_DEVICE, "/sys/class/sensordrv/kd_camera_hw/dev"),
    (ISP_DEVICE, "/sys/class/ispdrv/camera-isp/dev"),
    (ION_DEVICE, "/sys/class/misc/ion/dev"),
];

const fn iowr(nr: c_ulong, size: c_ulong) -> c_ulong {
    (3 << 30) | (size << 16) | ((b'i' as c_ulong) << 8) | nr
}
const fn io(nr: c_ulong) -> c_ulong {
    ((b'i' as c_ulong) << 8) | nr
}

// kd_camera_hw commands, as numbered by this kernel (they go up in steps of 5).
const SENSOR_OPEN: c_ulong = io(0);
const SENSOR_FEATURE: c_ulong = iowr(15, 16); // { camera; feature; *para; *len }
const SENSOR_CONTROL: c_ulong = iowr(20, 16); // { camera; scenario; *window; *config }
const SENSOR_CLOSE: c_ulong = io(25);
const SENSOR_SET_DRIVER: c_ulong = iowr(35, 8); // u32[2]: socket << 16 | list index
const SENSOR_SET_MCLK_PLL: c_ulong = iowr(60, 8); // { u8 on; u32 source }
const SENSOR_SET_CURRENT: c_ulong = iowr(70, 4); // must precede open
const FEATURE_SHUTTER: u32 = 3004;
const FEATURE_GAIN: u32 = 3006;

const ISP_RESET: c_ulong = 0x6B00;

const ION_ALLOC: c_ulong = (3 << 30) | (20 << 16) | ((b'I' as c_ulong) << 8);
const ION_FREE: c_ulong = (3 << 30) | (4 << 16) | ((b'I' as c_ulong) << 8) | 1;
const ION_SHARE: c_ulong = (3 << 30) | (8 << 16) | ((b'I' as c_ulong) << 8) | 4;
const ION_CUSTOM: c_ulong = (3 << 30) | (8 << 16) | ((b'I' as c_ulong) << 8) | 6;
const ION_HEAP_MULTIMEDIA: u32 = 0x400;
const ION_CMD_SYSTEM: u32 = 0;
const ION_CMD_MULTIMEDIA: u32 = 1;
const ION_SYS_GET_PHYS: u32 = 1;
const ION_MM_CONFIG_BUFFER: u32 = 0;
const M4U_PORT_IMGO: u32 = 15;

const ISP_BASE: i64 = 0x1500_0000;
const SENINF_BASE: i64 = 0x1500_8000;
const MIPI_BASE: i64 = 0x1021_5000;

// CAM (pass 1) registers, as offsets from ISP_BASE.
const CTL_EN1: usize = 0x4004;
const CTL_DMA_EN: usize = 0x400C;
const CTL_IMGO_FBC: usize = 0x40F4;
const IMGO_BASE: usize = 0x4300;
const TG_SEN_MODE: usize = 0x4410;
const TG_VF_CON: usize = 0x4414;
const TG_GRAB_PXL: usize = 0x4418;
const TG_GRAB_LIN: usize = 0x441C;
const TG_PATH_CFG: usize = 0x4420;
const TG_FRAME_COUNT: usize = 0x444C;
const CTL_FMT_SEL: usize = 0x4010;
/// Statistics modules (AF, flicker, AE/AWB): their output DMAs are not set up
/// here, and left on they stall the pass after about 20 lines.
const STATISTICS: u32 = 0x7_0000;

/// One camera's fixed setup.
pub struct Camera {
    socket: u32,
    list_index: u32,
    pub width: usize,
    pub height: usize,
    stride: usize,
    /// Which seninf (and MIPI port: CSI0 for 1, CSI1 for 2) the sensor is on.
    seninf: usize,
    /// MIPI data lanes minus one.
    lanes: u32,
    regs: &'static [(u16, u32)],
    limits: ExposureLimits,
    /// Rotation that gives the stock camera's portrait orientation.
    pub rotation: Rotation,
}

pub const REAR: Camera = Camera {
    socket: 1,
    list_index: 1,
    width: 2592,
    height: 1944,
    stride: 0xCB2,
    seninf: 1,
    lanes: 1,
    regs: camera_regs::REAR,
    // The frame is 2992 lines at 20 fps.
    limits: ExposureLimits { max_shutter: 2980, max_gain: 8 * UNIT_GAIN },
    rotation: Rotation::Clockwise90,
};

pub const FRONT: Camera = Camera {
    socket: 2,
    list_index: 0,
    width: 1600,
    height: 1200,
    stride: 0x7DA,
    seninf: 2,
    lanes: 0,
    regs: camera_regs::FRONT,
    limits: ExposureLimits { max_shutter: 1230, max_gain: 8 * UNIT_GAIN },
    rotation: Rotation::Clockwise270,
};

pub fn by_name(name: &str) -> Option<&'static Camera> {
    match name {
        "rear" => Some(&REAR),
        "front" => Some(&FRONT),
        _ => None,
    }
}

/// The last exposure each camera settled on, as a start for the next photo.
static LAST_EXPOSURE: Mutex<[Option<Exposure>; 2]> = Mutex::new([None, None]);
/// Only one photo at a time: both cameras share the ISP and the sensor driver.
static BUSY: Mutex<()> = Mutex::new(());

/// A finished photo and how it was taken.
pub struct Photo {
    pub jpeg: Vec<u8>,
    /// The developed image the JPEG was made from, for the panel.
    pub image: photo::Rgb,
    pub width: usize,
    pub height: usize,
    pub exposure: Exposure,
    pub mean_level: f32,
    pub frames: u32,
    pub elapsed: Duration,
}

/// Takes a photo and encodes it as a JPEG.
pub fn take(camera: &Camera, quality: u32) -> io::Result<Photo> {
    let _busy = BUSY.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let started = Instant::now();
    let slot = (camera.socket - 1) as usize;
    let start = LAST_EXPOSURE.lock().unwrap_or_else(|p| p.into_inner())[slot]
        .unwrap_or(Exposure { shutter: camera.limits.max_shutter / 2, gain: 2 * UNIT_GAIN });
    let frame = capture(camera, start)?;
    LAST_EXPOSURE.lock().unwrap_or_else(|p| p.into_inner())[slot] = Some(frame.exposure);
    let image = photo::develop(&frame.raw, camera.stride, camera.width, camera.height, camera.rotation);
    let jpeg = photo::encode_jpeg(&image, quality);
    Ok(Photo {
        jpeg,
        width: image.width,
        height: image.height,
        image,
        exposure: frame.exposure,
        mean_level: frame.mean_level,
        frames: frame.frames,
        elapsed: started.elapsed(),
    })
}

struct Frame {
    raw: Vec<u8>,
    exposure: Exposure,
    mean_level: f32,
    frames: u32,
}

/// Creates the device nodes the first time, from the numbers in sysfs.
fn ensure_nodes() -> io::Result<()> {
    for (path, number) in DEVICE_NUMBERS {
        if fs::metadata(path).is_ok() {
            continue;
        }
        let text = fs::read_to_string(number)?;
        let (major, minor) = crate::usb::parse_dev(&text)
            .ok_or_else(|| io::Error::other(format!("bad device number in {number}: {text}")))?;
        crate::pid1::make_char_device(path, major, minor, 0o600)?;
    }
    Ok(())
}

fn check(rc: c_int, what: &str) -> io::Result<()> {
    if rc < 0 {
        let err = io::Error::last_os_error();
        Err(io::Error::new(err.kind(), format!("{what}: {err}")))
    } else {
        Ok(())
    }
}

fn call(file: &File, request: c_ulong, argument: *mut c_void, what: &str) -> io::Result<()> {
    // SAFETY: callers pass a buffer of the size the request reads and writes,
    // valid for the duration of the call.
    check(unsafe { ioctl(file.as_raw_fd(), request as c_int, argument) }, what)
}

fn call_plain(file: &File, request: c_ulong, what: &str) -> io::Result<()> {
    // SAFETY: these requests take no argument.
    check(unsafe { ioctl(file.as_raw_fd(), request as c_int, 0usize) }, what)
}

/// A block of hardware registers mapped through the ISP driver, which accepts
/// only its own bases and only read-write mappings.
struct Registers {
    base: *mut u32,
    len: usize,
}

impl Registers {
    fn map(file: &File, physical: i64, len: usize) -> io::Result<Self> {
        // SAFETY: a new mapping chosen by the kernel touches no existing memory;
        // the result is checked before use.
        let base = unsafe {
            mmap(ptr::null_mut(), len, PROT_READ | PROT_WRITE, MAP_SHARED, file.as_raw_fd(), physical)
        };
        if base as isize == -1 {
            return Err(io::Error::new(
                io::Error::last_os_error().kind(),
                format!("map registers at {physical:#x}: {}", io::Error::last_os_error()),
            ));
        }
        Ok(Registers { base: base.cast(), len })
    }

    fn read(&self, offset: usize) -> u32 {
        assert!(offset + 4 <= self.len);
        // SAFETY: the offset is inside the mapping; device registers need
        // volatile 32-bit accesses.
        unsafe { ptr::read_volatile(self.base.add(offset / 4)) }
    }

    fn write(&self, offset: usize, value: u32) {
        assert!(offset + 4 <= self.len);
        // SAFETY: as in `read`.
        unsafe { ptr::write_volatile(self.base.add(offset / 4), value) }
    }

    fn set(&self, offset: usize, bits: u32) {
        self.write(offset, self.read(offset) | bits);
    }

    fn clear(&self, offset: usize, bits: u32) {
        self.write(offset, self.read(offset) & !bits);
    }

    fn field(&self, offset: usize, shift: u32, width: u32, value: u32) {
        let mask = ((1u32 << width) - 1) << shift;
        self.write(offset, (self.read(offset) & !mask) | ((value << shift) & mask));
    }
}

impl Drop for Registers {
    fn drop(&mut self) {
        // SAFETY: the mapping came from `mmap` and nothing uses it after this.
        unsafe { munmap(self.base.cast(), self.len) };
    }
}

/// A buffer from the multimedia ION heap, mapped for the ISP (through the M4U)
/// and for this process.
struct IonBuffer<'a> {
    ion: &'a File,
    handle: i32,
    share: File,
    cpu: *mut u8,
    len: usize,
    device_address: u32,
}

impl<'a> IonBuffer<'a> {
    fn new(ion: &'a File, len: usize, port: u32) -> io::Result<Self> {
        let mut alloc = [len as u32, 0, ION_HEAP_MULTIMEDIA, 0, 0];
        call(ion, ION_ALLOC, alloc.as_mut_ptr().cast(), "ion alloc")?;
        let handle = alloc[4] as i32;
        // From here on the handle is freed by `drop` if a later step fails.
        let mut buffer = IonBuffer { ion, handle, share: File::open("/dev/null")?, cpu: ptr::null_mut(), len, device_address: 0 };
        let mut config = [ION_MM_CONFIG_BUFFER, handle as u32, port, 0, 0, 0, 0, 0];
        buffer.custom(ION_CMD_MULTIMEDIA, &mut config, "ion config buffer")?;
        let mut phys = [ION_SYS_GET_PHYS, handle as u32, 0, len as u32, 0, 0, 0, 0];
        buffer.custom(ION_CMD_SYSTEM, &mut phys, "ion get address")?;
        buffer.device_address = phys[2];
        let mut share = [handle, -1];
        call(ion, ION_SHARE, share.as_mut_ptr().cast(), "ion share")?;
        // SAFETY: the kernel just returned this new file descriptor to us.
        buffer.share = unsafe { <File as std::os::fd::FromRawFd>::from_raw_fd(share[1]) };
        // SAFETY: as in Registers::map.
        let cpu = unsafe {
            mmap(ptr::null_mut(), len, PROT_READ | PROT_WRITE, MAP_SHARED, buffer.share.as_raw_fd(), 0)
        };
        if cpu as isize == -1 {
            return Err(io::Error::other(format!("map ion buffer: {}", io::Error::last_os_error())));
        }
        buffer.cpu = cpu.cast();
        Ok(buffer)
    }

    fn custom(&self, command: u32, data: &mut [u32; 8], what: &str) -> io::Result<()> {
        let mut argument = [command, data.as_mut_ptr() as u32];
        call(self.ion, ION_CUSTOM, argument.as_mut_ptr().cast(), what)
    }

    fn bytes(&self) -> &[u8] {
        // SAFETY: the mapping is `len` bytes and lives as long as `self`. The ISP
        // may still write to it; callers stop the ISP first or accept tearing.
        unsafe { std::slice::from_raw_parts(self.cpu, self.len) }
    }
}

impl Drop for IonBuffer<'_> {
    fn drop(&mut self) {
        if !self.cpu.is_null() {
            // SAFETY: the mapping came from `mmap` and nothing uses it after this.
            unsafe { munmap(self.cpu.cast(), self.len) };
        }
        let mut handle = [self.handle];
        let _ = call(self.ion, ION_FREE, handle.as_mut_ptr().cast(), "ion free");
    }
}

struct Sensor<'a> {
    file: &'a File,
    socket: u32,
}

impl Sensor<'_> {
    fn feature(&self, feature: u32, value: u32) -> io::Result<()> {
        // The drivers read these values as 64-bit numbers.
        let mut para = [value, 0u32];
        let mut len: u32 = 8;
        let mut argument = [self.socket, feature, para.as_mut_ptr() as u32, &mut len as *mut u32 as u32];
        call(self.file, SENSOR_FEATURE, argument.as_mut_ptr().cast(), "sensor feature")
    }

    fn set_exposure(&self, exposure: Exposure) -> io::Result<()> {
        self.feature(FEATURE_SHUTTER, exposure.shutter)?;
        self.feature(FEATURE_GAIN, exposure.gain)
    }
}

/// The sensor clock: the PLL through the sensor driver, then the TG1 and TG2
/// clock outputs in seninf, as SeninfDrvImp::setMclk1/2 do for 24 MHz.
fn mclk(sensor: &File, seninf: &Registers, on: bool) -> io::Result<()> {
    let mut pll = [u32::from(on), 1u32]; // { on; source 1 = 48 MHz }
    call(sensor, SENSOR_SET_MCLK_PLL, pll.as_mut_ptr().cast(), "mclk pll")?;
    for (phase, clock) in [(0x200, 0x204), (0x600, 0x604)] {
        if !on {
            seninf.clear(phase, 1 << 31);
            continue;
        }
        seninf.set(phase, 1 << 31); // PCEN
        seninf.set(0x000, 0xC00);
        seninf.write(clock, (seninf.read(clock) & !0x3F_3F3F) | 1 | (1 << 16)); // 48 MHz / 2
        let mut value = seninf.read(phase);
        value &= !((1 << 2) | (1 << 28) | (1 << 6) | 3);
        value |= 1; // TGCLK_SEL
        seninf.write(phase, value);
    }
    Ok(())
}

/// Seninf setup for a MIPI sensor on seninf `n` through the NCSI2 receiver:
/// setSeninfNCtrl, the MIPI analog calibration and setSeninfNNCSI2 for RAW10.
fn seninf_ncsi2(seninf: &Registers, mipi: &Registers, n: usize, lanes: u32) {
    let s = 0x400 * (n - 1);
    let a = 0x800 + 0x400 * (n - 1);
    seninf.set(s + 0x100, 1);
    seninf.field(s + 0x100, 28, 3, 0); // 10-bit pad
    seninf.field(s + 0x100, 12, 4, 8); // source: MIPI
    for offset in [76, 80] {
        mipi.write(a + offset, mipi.read(a + offset) & 0xFEFB_EFBE);
    }
    for offset in [0, 4, 8, 12, 16] {
        mipi.set(a + offset, 8);
    }
    mipi.set(a + 36, 1);
    thread::sleep(Duration::from_micros(30));
    mipi.set(a + 32, 3);
    thread::sleep(Duration::from_micros(1));
    for offset in [0, 4, 8, 12, 16] {
        mipi.set(a + offset, 1);
    }
    for bit in [0x10, 1, 2, 4, 8] {
        seninf.set(s + 0x3D8, bit);
    }
    seninf.set(s + 0x338, 1);
    seninf.write(s + 0x33C, 0x1541);
    seninf.set(s + 0x338, 4);
    thread::sleep(Duration::from_micros(500));
    seninf.clear(s + 0x338, 1);
    for bit in [0x10, 1, 2, 4, 8] {
        seninf.clear(s + 0x3D8, bit);
    }
    let settle = 85 * 300 / 1000;
    seninf.write(s + 0x3AC, 0);
    seninf.write(s + 0x100, (seninf.read(s + 0x100) & !0xF000) | 0x8000);
    seninf.clear(s + 0x3A0, 0x80);
    seninf.write(s + 0x3A8, (settle << 8) & 0xFFFF);
    seninf.set(s + 0x3A0, ((1 << (lanes + 1)) - 1) | (1 << 16) | 0x400_0000 | 0x10);
    seninf.write(s + 0x3B0, 0x8000_7FFF);
}

/// Mux 0 takes MIPI data in RAW format and feeds the ISP's TG1
/// (SeninfDrvImp::setSeninf1MuxCtrl and setSeninfTopMuxCtrl).
fn seninf_mux(seninf: &Registers, from: usize) {
    let r = 0x120;
    seninf.set(r, 1 << 31);
    seninf.field(r, 12, 4, 8);
    seninf.clear(r, 0x100);
    seninf.write(r, (seninf.read(r) & !0x3000_0000) | 0x1000_0000);
    seninf.write(r, (seninf.read(r) & !0x0FC0_0000) | 0x0EC0_0000);
    seninf.set(r, 0x3F_0000);
    seninf.field(r, 10, 1, 0);
    seninf.field(r, 9, 1, 0);
    seninf.write(0x8, (seninf.read(0x8) & 0xFFF0) | (from as u32 - 1));
}

fn frame_count(isp: &Registers) -> u32 {
    isp.read(TG_FRAME_COUNT) >> 24
}

/// Waits until `count` more frames have been written, or about a second each.
fn wait_frames(isp: &Registers, count: u32) -> io::Result<u32> {
    let start = frame_count(isp);
    let deadline = Instant::now() + Duration::from_millis(1000 * u64::from(count) + 500);
    loop {
        let done = frame_count(isp).wrapping_sub(start) & 0xFF;
        if done >= count {
            return Ok(done);
        }
        if Instant::now() > deadline {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "the camera sends no frames"));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn capture(camera: &Camera, start: Exposure) -> io::Result<Frame> {
    ensure_nodes()?;
    let isp_file = OpenOptions::new().read(true).write(true).open(ISP_DEVICE)?;
    let sensor_file = OpenOptions::new().read(true).write(true).open(SENSOR_DEVICE)?;
    let ion = OpenOptions::new().read(true).write(true).open(ION_DEVICE)?;
    call_plain(&isp_file, ISP_RESET, "isp reset")?;
    let isp = Registers::map(&isp_file, ISP_BASE, 0x1_0000)?;
    let seninf = Registers::map(&isp_file, SENINF_BASE, 0x4000)?;
    let mipi = Registers::map(&isp_file, MIPI_BASE, 0x3000)?;
    let buffer = IonBuffer::new(&ion, camera.stride * camera.height, M4U_PORT_IMGO)?;

    // SeninfDrvImp::init, then the sensor clock.
    for offset in [0x120, 0x520, 0x920, 0xD20] {
        seninf.set(offset, 1 << 31);
    }
    for offset in [0x100, 0x500, 0x900] {
        seninf.set(offset, 1);
    }
    mclk(&sensor_file, &seninf, true)?;

    let sensor = Sensor { file: &sensor_file, socket: camera.socket };
    let result = (|| {
        let mut drivers = [(camera.socket << 16) | camera.list_index, 0u32];
        call(&sensor_file, SENSOR_SET_DRIVER, drivers.as_mut_ptr().cast(), "sensor set driver")?;
        let mut current = camera.socket;
        call(&sensor_file, SENSOR_SET_CURRENT, (&mut current as *mut u32).cast(), "sensor set current")?;
        call_plain(&sensor_file, SENSOR_OPEN, "sensor open")?;
        stream(camera, &sensor, &isp, &seninf, &mipi, &buffer, start)
    })();

    isp.clear(TG_VF_CON, 1);
    isp.write(CTL_DMA_EN, 0);
    isp.write(CTL_EN1, 0);
    let _ = call_plain(&sensor_file, SENSOR_CLOSE, "sensor close");
    let _ = mclk(&sensor_file, &seninf, false);
    result
}

fn stream(
    camera: &Camera,
    sensor: &Sensor,
    isp: &Registers,
    seninf: &Registers,
    mipi: &Registers,
    buffer: &IonBuffer,
    start: Exposure,
) -> io::Result<Frame> {
    // HalSensor::seninfControl for this sensor.
    isp.field(TG_GRAB_PXL, 16, 15, camera.width as u32);
    isp.field(TG_GRAB_PXL, 0, 15, 0);
    isp.field(TG_GRAB_LIN, 16, 13, camera.height as u32);
    isp.field(TG_GRAB_LIN, 0, 13, 0);
    isp.clear(TG_PATH_CFG, 3);
    isp.field(CTL_FMT_SEL, 16, 3, 0); // TG1 format: RAW
    seninf_mux(seninf, camera.seninf);
    isp.set(TG_SEN_MODE, 1 | 4);
    isp.set(TG_VF_CON, 0x1000);
    isp.clear(TG_VF_CON, 2);
    seninf_ncsi2(seninf, mipi, camera.seninf, camera.lanes);

    // Sensor: preview scenario, which starts streaming, and a first exposure.
    let mut window = [0u8; 256];
    let mut config = [0u8; 256];
    let mut control = [camera.socket, 0, window.as_mut_ptr() as u32, config.as_mut_ptr() as u32];
    call(sensor.file, SENSOR_CONTROL, control.as_mut_ptr().cast(), "sensor preview")?;
    let mut exposure = start;
    sensor.set_exposure(exposure)?;

    // ISP pass 1 as stock Android set it, writing raw frames to our buffer only.
    for &(offset, value) in camera.regs {
        let offset = usize::from(offset);
        if ![0x4000, CTL_DMA_EN, IMGO_BASE, TG_SEN_MODE, TG_VF_CON].contains(&offset) {
            isp.write(offset, value);
        }
    }
    isp.clear(CTL_EN1, STATISTICS);
    isp.write(CTL_IMGO_FBC, 0);
    isp.write(IMGO_BASE, buffer.device_address);
    isp.write(CTL_DMA_EN, 1); // IMGO
    isp.set(TG_VF_CON, 1);

    // Auto exposure: measure, adjust, let the change reach a frame, repeat.
    let mut frames = wait_frames(isp, 2)?;
    let mut mean = 0.0;
    for _ in 0..8 {
        frames += wait_frames(isp, 1)?;
        mean = photo::mean_level(buffer.bytes(), camera.stride, camera.width, camera.height, 16);
        if photo::exposure_settled(mean, exposure, camera.limits) {
            break;
        }
        exposure = photo::next_exposure(exposure, mean, camera.limits);
        sensor.set_exposure(exposure)?;
        // The sensors apply a new exposure one or two frames later.
        frames += wait_frames(isp, 2)?;
    }

    // Stop after the frame in progress, so the buffer holds one whole frame.
    frames += wait_frames(isp, 1)?;
    isp.clear(TG_VF_CON, 1);
    thread::sleep(Duration::from_millis(150));
    Ok(Frame { raw: buffer.bytes().to_vec(), exposure, mean_level: mean, frames })
}
