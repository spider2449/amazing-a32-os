//! USB serial gadget setup through the kernel's android_usb sysfs interface.
//!
//! The write sequence mirrors `sys.usb.config=acm` in the stock
//! `init.mt6735.usb.rc`, with the vendor ID the stock firmware reports.

use std::fs;
use std::io;
use std::path::Path;

#[cfg(target_os = "linux")]
pub const ANDROID_USB: &str = "/sys/class/android_usb/android0";
#[cfg(target_os = "linux")]
pub const SERIAL_SYSFS_DEV: &str = "/sys/class/tty/ttyGS0/dev";

/// Sysfs writes as (attribute, value), in order.
const ACM_SEQUENCE: &[(&str, &str)] = &[
    ("enable", "0"),
    ("idVendor", "0e8d"),
    ("idProduct", "2007"),
    ("f_acm/instances", "1"),
    ("functions", "acm"),
    ("bDeviceClass", "02"),
    ("enable", "1"),
];

/// Applies the sequence under `root`, stopping at the first failed write.
pub fn enable_serial(root: &Path) -> io::Result<()> {
    for (attribute, value) in ACM_SEQUENCE {
        fs::write(root.join(attribute), value)
            .map_err(|err| io::Error::new(err.kind(), format!("{attribute}: {err}")))?;
    }
    Ok(())
}

/// Parses a sysfs `major:minor` device number.
pub fn parse_dev(text: &str) -> Option<(u32, u32)> {
    let (major, minor) = text.trim().split_once(':')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_final_values_in_order() {
        let root = std::env::temp_dir().join(format!("a32-usb-test-{}", std::process::id()));
        fs::create_dir_all(root.join("f_acm")).unwrap();

        enable_serial(&root).unwrap();

        let read = |name: &str| fs::read_to_string(root.join(name)).unwrap();
        assert_eq!(read("enable"), "1");
        assert_eq!(read("functions"), "acm");
        assert_eq!(read("idVendor"), "0e8d");
        assert_eq!(read("f_acm/instances"), "1");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn reports_the_failing_attribute() {
        let root = std::env::temp_dir().join(format!("a32-usb-missing-{}", std::process::id()));
        let err = enable_serial(&root).unwrap_err();
        assert!(err.to_string().starts_with("enable:"), "{err}");
    }

    #[test]
    fn parses_device_numbers() {
        assert_eq!(parse_dev("234:0\n"), Some((234, 0)));
        assert_eq!(parse_dev("234"), None);
        assert_eq!(parse_dev("a:b"), None);
    }
}
