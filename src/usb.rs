//! USB serial gadget setup through the kernel's android_usb sysfs interface.
//!
//! The write sequence mirrors `sys.usb.config=acm` in the stock
//! `init.mt6735.usb.rc`, with the vendor ID the stock firmware reports.

use std::fs;
use std::io;
#[cfg(target_os = "linux")]
use std::net::Ipv4Addr;
use std::path::Path;

#[cfg(target_os = "linux")]
pub const ANDROID_USB: &str = "/sys/class/android_usb/android0";
#[cfg(target_os = "linux")]
pub const SERIAL_SYSFS_DEV: &str = "/sys/class/tty/ttyGS0/dev";

/// The network interface the kernel makes for the RNDIS function.
#[cfg(target_os = "linux")]
pub const NETWORK_INTERFACE: &str = "rndis0";
#[cfg(target_os = "linux")]
pub const PHONE_ADDRESS: Ipv4Addr = Ipv4Addr::new(192, 168, 42, 1);
#[cfg(target_os = "linux")]
pub const COMPUTER_ADDRESS: Ipv4Addr = Ipv4Addr::new(192, 168, 42, 2);
#[cfg(target_os = "linux")]
pub const NETMASK: Ipv4Addr = Ipv4Addr::new(255, 255, 255, 0);
#[cfg(target_os = "linux")]
pub const PREFIX_LENGTH: u32 = 24;

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

/// A network adapter next to the serial port. The product ID differs from the
/// serial-only one so that Windows does not reuse that device's driver choice, and
/// the device class is 00 so that Windows treats it as a composite device.
#[cfg(any(target_os = "linux", test))]
const RNDIS_ACM_SEQUENCE: &[(&str, &str)] = &[
    ("enable", "0"),
    ("idVendor", "0e8d"),
    ("idProduct", "2099"),
    ("bDeviceClass", "00"),
    ("f_acm/instances", "1"),
    ("f_rndis/wceis", "1"),
    ("f_rndis/manufacturer", "Amazing"),
    ("f_rndis/ethaddr", "02:a3:20:00:00:01"),
    ("functions", "rndis,acm"),
    ("enable", "1"),
];

/// Applies the serial-only sequence under `root`, stopping at the first failed write.
pub fn enable_serial(root: &Path) -> io::Result<()> {
    apply(root, ACM_SEQUENCE)
}

/// Applies the sequence with the network adapter, stopping at the first failed write.
#[cfg(any(target_os = "linux", test))]
pub fn enable_serial_and_network(root: &Path) -> io::Result<()> {
    apply(root, RNDIS_ACM_SEQUENCE)
}

fn apply(root: &Path, sequence: &[(&str, &str)]) -> io::Result<()> {
    for (attribute, value) in sequence {
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
    fn the_network_sequence_keeps_the_serial_port() {
        let root = std::env::temp_dir().join(format!("a32-usb-net-{}", std::process::id()));
        fs::create_dir_all(root.join("f_acm")).unwrap();
        fs::create_dir_all(root.join("f_rndis")).unwrap();

        enable_serial_and_network(&root).unwrap();

        let read = |name: &str| fs::read_to_string(root.join(name)).unwrap();
        assert_eq!(read("enable"), "1");
        assert_eq!(read("functions"), "rndis,acm");
        assert_eq!(read("bDeviceClass"), "00");
        assert_eq!(read("idProduct"), "2099");
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
