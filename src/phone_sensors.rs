//! The phone's own accelerometer, light and proximity sensors, read through the
//! stock kernel's MediaTek drivers. The BLE sensors are in `sensors`.
//!
//! The light and proximity sensors share a chip and disturb each other when both are
//! on, so each is switched on alone for one reading. A reading takes about 2 s.
//!
//! The driver always returns 0 for the accelerometer's Z axis. Its raw output has only
//! the low 8 bits of Z, which wrap once per g and drift, so Z is reported as unknown.

#[cfg(target_os = "linux")]
use std::fs::{self, File};
#[cfg(target_os = "linux")]
use std::io;
#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
#[cfg(target_os = "linux")]
use std::os::raw::{c_int, c_void};
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use std::sync::Mutex;
#[cfg(target_os = "linux")]
use std::thread;
#[cfg(target_os = "linux")]
use std::time::Duration;

// In a test, a finger over the sensor read about 600 and nothing near it at most 15.
const NEAR_THRESHOLD: u32 = 300;

#[derive(Debug, PartialEq)]
pub struct Reading {
    /// Acceleration in mm/s², which includes gravity.
    pub x: i32,
    pub y: i32,
    /// The driver's values, which are not lux.
    pub light: u32,
    pub proximity: u32,
}

impl Reading {
    fn near(&self) -> bool {
        self.proximity >= NEAR_THRESHOLD
    }

    pub fn text(&self) -> String {
        format!(
            "acceleration x: {:.2} m/s2\nacceleration y: {:.2} m/s2\nacceleration z: not available\nlight: {} (raw)\nproximity: {} (raw), {}",
            f64::from(self.x) / 1000.0,
            f64::from(self.y) / 1000.0,
            self.light,
            self.proximity,
            if self.near() { "near" } else { "far" },
        )
    }

    pub fn json(&self) -> String {
        format!(
            "{{\"acceleration\":{{\"x\":{:.3},\"y\":{:.3},\"z\":null}},\"light_raw\":{},\"proximity_raw\":{},\"near\":{}}}\n",
            f64::from(self.x) / 1000.0,
            f64::from(self.y) / 1000.0,
            self.light,
            self.proximity,
            self.near(),
        )
    }
}

/// Parses the driver's `x y z` text in hexadecimal, such as `fffffdc2 00bf 0000`, where
/// a negative number has all its 32 bits. Returns X and Y.
fn parse_acceleration(text: &str) -> Option<(i32, i32)> {
    let mut fields = text.split_whitespace().map(|field| u32::from_str_radix(field, 16).ok().map(|bits| bits as i32));
    let (x, y) = (fields.next()??, fields.next()??);
    fields.next()??;
    fields.next().is_none().then_some((x, y))
}

/// Parses text that ends in a number, such as `als raw dat= 29`.
fn parse_last_number(text: &str) -> Option<u32> {
    text.split_whitespace().last()?.parse().ok()
}

#[cfg(target_os = "linux")]
const ACCELEROMETER: &str = "/dev/gsensor";
#[cfg(target_os = "linux")]
const ACCELEROMETER_SYSFS_DEV: &str = "/sys/class/misc/gsensor/dev";
// _IOR(0x85, 3, int) in the MediaTek sensor headers, which fills a text buffer.
#[cfg(target_os = "linux")]
const READ_SENSOR_DATA: u32 = 0x8004_8503;
#[cfg(target_os = "linux")]
const SWITCHES: &str = "/sys/class/misc/m_alsps_misc";
#[cfg(target_os = "linux")]
const VALUES: &str = "/sys/bus/platform/drivers/als_ps";
// The first reading after switching on was a little low; later ones had settled.
#[cfg(target_os = "linux")]
const SETTLE: Duration = Duration::from_secs(1);
/// One reading at a time, because a reading switches sensors on and off.
#[cfg(target_os = "linux")]
static READING: Mutex<()> = Mutex::new(());

#[cfg(target_os = "linux")]
extern "C" {
    fn ioctl(fd: c_int, request: c_int, ...) -> c_int;
}

#[cfg(target_os = "linux")]
pub fn read() -> io::Result<Reading> {
    let _only_one = READING.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let (x, y) = read_acceleration()?;
    let light = read_switched("alsactive", "als")?;
    let proximity = read_switched("psactive", "ps")?;
    Ok(Reading { x, y, light, proximity })
}

#[cfg(target_os = "linux")]
fn read_acceleration() -> io::Result<(i32, i32)> {
    if !Path::new(ACCELEROMETER).exists() {
        let text = fs::read_to_string(ACCELEROMETER_SYSFS_DEV)?;
        let (major, minor) =
            crate::usb::parse_dev(&text).ok_or_else(|| io::Error::other(format!("bad device number: {text}")))?;
        crate::pid1::make_char_device(ACCELEROMETER, major, minor, 0o600)?;
    }
    let device = File::open(ACCELEROMETER)?;
    let mut buffer = [0u8; 64];
    // SAFETY: the buffer outlives the call and is larger than the driver's text.
    let rc = unsafe { ioctl(device.as_raw_fd(), READ_SENSOR_DATA as c_int, buffer.as_mut_ptr().cast::<c_void>()) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    let end = buffer.iter().position(|&byte| byte == 0).unwrap_or(buffer.len());
    let text = String::from_utf8_lossy(&buffer[..end]);
    parse_acceleration(&text).ok_or_else(|| io::Error::other(format!("bad accelerometer data: {text}")))
}

/// Switches one sensor on, reads it once it has settled, and always switches it off.
#[cfg(target_os = "linux")]
fn read_switched(switch: &str, value: &str) -> io::Result<u32> {
    let switch = format!("{SWITCHES}/{switch}");
    fs::write(&switch, "1")?;
    thread::sleep(SETTLE);
    let text = fs::read_to_string(format!("{VALUES}/{value}"));
    fs::write(&switch, "0")?;
    let text = text?;
    parse_last_number(&text).ok_or_else(|| io::Error::other(format!("bad sensor data: {text}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_negative_and_positive_acceleration() {
        assert_eq!(parse_acceleration("fffffdc2 00bf 0000"), Some((-574, 191)));
        assert_eq!(parse_acceleration("1923 ffffffd9 0000"), Some((6435, -39)));
    }

    #[test]
    fn rejects_malformed_acceleration() {
        assert_eq!(parse_acceleration(""), None);
        assert_eq!(parse_acceleration("00bf 0000"), None);
        assert_eq!(parse_acceleration("00bf zz 0000"), None);
        assert_eq!(parse_acceleration("1 2 3 4"), None);
    }

    #[test]
    fn parses_the_number_at_the_end_of_the_drivers_text() {
        assert_eq!(parse_last_number("als raw dat= 29\n"), Some(29));
        assert_eq!(parse_last_number("ps raw dat= 0"), Some(0));
        assert_eq!(parse_last_number("nothing"), None);
        assert_eq!(parse_last_number(""), None);
    }

    fn reading(proximity: u32) -> Reading {
        Reading { x: -574, y: 9650, light: 28, proximity }
    }

    #[test]
    fn reports_z_as_unknown() {
        assert!(reading(0).text().contains("acceleration z: not available"));
        assert!(reading(0).json().contains("\"z\":null"));
    }

    #[test]
    fn proximity_is_near_from_the_threshold() {
        assert!(reading(NEAR_THRESHOLD - 1).text().ends_with(", far"));
        assert!(reading(NEAR_THRESHOLD).text().ends_with(", near"));
        assert!(reading(600).json().contains("\"near\":true"));
    }

    #[test]
    fn json_has_the_values_in_metres_per_second_squared() {
        assert_eq!(
            reading(5).json(),
            "{\"acceleration\":{\"x\":-0.574,\"y\":9.650,\"z\":null},\"light_raw\":28,\"proximity_raw\":5,\"near\":false}\n"
        );
    }
}
