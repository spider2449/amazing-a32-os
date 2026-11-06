//! A BLE scanner that talks HCI straight to MediaTek's `/dev/stpbt`, because this
//! kernel has no Bluetooth stack. It only listens: it never advertises, connects or
//! sends anything over the air beyond what a passive scan needs, which is nothing.
//! Packets are H4: a type byte, then an HCI command or event. Nothing here does
//! I/O, so it runs on the host in unit tests.

use crate::sensors::{self, Reading};
use std::collections::BTreeMap;
use std::time::Duration;

const H4_COMMAND: u8 = 0x01;
const H4_EVENT: u8 = 0x04;
const EVENT_COMMAND_COMPLETE: u8 = 0x0E;
const EVENT_COMMAND_STATUS: u8 = 0x0F;
const EVENT_LE_META: u8 = 0x3E;
const LE_ADVERTISING_REPORT: u8 = 0x02;

pub const RESET: u16 = 0x0C03;
const SET_EVENT_MASK: u16 = 0x0C01;
const LE_SET_EVENT_MASK: u16 = 0x2001;
const LE_SET_SCAN_PARAMETERS: u16 = 0x200B;
pub const LE_SET_SCAN_ENABLE: u16 = 0x200C;

// In 0.625 ms units: listen 30 ms of every 300 ms. Listening 30 ms of every 1.28 s
// cost Wi-Fi as much, so the shorter interval, which hears devices more often, stays.
// Bluetooth shares the 2.4 GHz radio with Wi-Fi either way.
const SCAN_INTERVAL: u16 = 480;
const SCAN_WINDOW: u16 = 48;

/// A device not heard from for this long is dropped from the list.
pub const FORGET_AFTER: Duration = Duration::from_secs(600);
/// Busy places can have hundreds of devices, mostly with changing random addresses.
pub const DEVICE_LIMIT: usize = 256;
const NAME_LIMIT: usize = 64;

pub fn command(opcode: u16, parameters: &[u8]) -> Vec<u8> {
    let [low, high] = opcode.to_le_bytes();
    let mut packet = vec![H4_COMMAND, low, high, parameters.len() as u8];
    packet.extend_from_slice(parameters);
    packet
}

/// The commands that start a passive scan, in order, each answered by a Command
/// Complete event.
pub fn start_scan() -> Vec<(u16, Vec<u8>)> {
    let [interval_low, interval_high] = SCAN_INTERVAL.to_le_bytes();
    let [window_low, window_high] = SCAN_WINDOW.to_le_bytes();
    vec![
        (RESET, vec![]),
        // All events, which includes LE Meta. The default mask leaves it out.
        (SET_EVENT_MASK, vec![0xFF; 8]),
        // Advertising reports and the other LE events of Bluetooth 4.0.
        (LE_SET_EVENT_MASK, vec![0x1F, 0, 0, 0, 0, 0, 0, 0]),
        // Passive, public own address, accept all advertisers.
        (LE_SET_SCAN_PARAMETERS, vec![0x00, interval_low, interval_high, window_low, window_high, 0x00, 0x00]),
        // On, and repeats kept so that the signal strength stays current.
        (LE_SET_SCAN_ENABLE, vec![0x01, 0x00]),
    ]
}

#[derive(Debug, PartialEq)]
pub enum Event {
    /// Opcode and status of a finished command. Status 0 is success.
    Complete(u16, u8),
    Advertisements(Vec<Advertisement>),
    Other,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Advertisement {
    pub address: [u8; 6],
    pub random: bool,
    pub rssi: i8,
    pub name: Option<String>,
    pub sensor: Option<Reading>,
}

/// Splits a byte stream into events. Reads from the device can hold part of an
/// event, or several.
#[derive(Default)]
pub struct Parser {
    pending: Vec<u8>,
}

impl Parser {
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Event> {
        self.pending.extend_from_slice(bytes);
        let mut events = Vec::new();
        loop {
            // Skip anything that is not an event, so one bad byte cannot stop parsing.
            match self.pending.iter().position(|&byte| byte == H4_EVENT) {
                Some(0) => {}
                Some(start) => {
                    self.pending.drain(..start);
                }
                None => {
                    self.pending.clear();
                    break;
                }
            }
            if self.pending.len() < 3 {
                break;
            }
            let length = usize::from(self.pending[2]);
            if self.pending.len() < 3 + length {
                break;
            }
            let packet: Vec<u8> = self.pending.drain(..3 + length).collect();
            events.push(parse_event(packet[1], &packet[3..]));
        }
        events
    }
}

fn parse_event(code: u8, payload: &[u8]) -> Event {
    match code {
        EVENT_COMMAND_COMPLETE if payload.len() >= 4 => {
            Event::Complete(u16::from_le_bytes([payload[1], payload[2]]), payload[3])
        }
        EVENT_COMMAND_STATUS if payload.len() >= 4 => {
            Event::Complete(u16::from_le_bytes([payload[2], payload[3]]), payload[0])
        }
        EVENT_LE_META if payload.first() == Some(&LE_ADVERTISING_REPORT) => {
            Event::Advertisements(parse_reports(&payload[1..]))
        }
        _ => Event::Other,
    }
}

/// Reads the reports of an LE Advertising Report event. Each report is: event
/// type, address type, address, data length, data, then RSSI.
fn parse_reports(body: &[u8]) -> Vec<Advertisement> {
    let Some((&count, mut rest)) = body.split_first() else {
        return Vec::new();
    };
    let mut reports = Vec::new();
    for _ in 0..count {
        if rest.len() < 9 {
            break;
        }
        let random = rest[1] & 1 == 1;
        let mut address = [0u8; 6];
        address.copy_from_slice(&rest[2..8]);
        let data_length = usize::from(rest[8]);
        if rest.len() < 10 + data_length {
            break;
        }
        let data = &rest[9..9 + data_length];
        reports.push(Advertisement {
            address,
            random,
            rssi: rest[9 + data_length] as i8,
            name: local_name(data),
            sensor: sensors::parse(data),
        });
        rest = &rest[10 + data_length..];
    }
    reports
}

/// Finds the complete or shortened local name in advertising data.
pub fn local_name(mut data: &[u8]) -> Option<String> {
    let mut shortened = None;
    while let Some((&length, rest)) = data.split_first() {
        let length = usize::from(length);
        if length == 0 || rest.len() < length {
            break;
        }
        let (kind, value) = (rest[0], &rest[1..length]);
        let text = || {
            let text: String = String::from_utf8_lossy(value)
                .chars()
                .filter(|character| !character.is_control())
                .take(NAME_LIMIT)
                .collect();
            Some(text).filter(|text| !text.is_empty())
        };
        match kind {
            0x09 => return text(),
            0x08 => shortened = text(),
            _ => {}
        }
        data = &rest[length..];
    }
    shortened
}

/// Formats an address the usual way, most significant byte first.
pub fn address_text(address: &[u8; 6]) -> String {
    address.iter().rev().map(|byte| format!("{byte:02x}")).collect::<Vec<_>>().join(":")
}

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub address: [u8; 6],
    pub random: bool,
    pub name: Option<String>,
    pub rssi: i8,
    pub last_seen: Duration,
    pub reports: u64,
    /// The latest values from a sensor, merged across advertisements, and when the
    /// last one came.
    pub sensor: Option<(Reading, Duration)>,
}

/// The devices heard recently.
#[derive(Default)]
pub struct Devices {
    by_address: BTreeMap<[u8; 6], Device>,
}

impl Devices {
    pub const fn new() -> Self {
        Self { by_address: BTreeMap::new() }
    }

    pub fn record(&mut self, advertisement: &Advertisement, now: Duration) {
        if !self.by_address.contains_key(&advertisement.address) && self.by_address.len() >= DEVICE_LIMIT {
            self.forget_old(now);
            if self.by_address.len() >= DEVICE_LIMIT {
                // Make room by dropping the device heard from longest ago.
                if let Some(oldest) = self.by_address.values().min_by_key(|device| device.last_seen).map(|device| device.address) {
                    self.by_address.remove(&oldest);
                }
            }
        }
        let device = self.by_address.entry(advertisement.address).or_insert_with(|| Device {
            address: advertisement.address,
            random: advertisement.random,
            name: None,
            rssi: advertisement.rssi,
            last_seen: now,
            reports: 0,
            sensor: None,
        });
        device.rssi = advertisement.rssi;
        device.last_seen = now;
        device.reports += 1;
        // A scan response may carry the name while the advertisement does not.
        if advertisement.name.is_some() {
            device.name.clone_from(&advertisement.name);
        }
        if let Some(reading) = &advertisement.sensor {
            match &mut device.sensor {
                Some((kept, updated)) => {
                    kept.merge(reading);
                    *updated = now;
                }
                None => device.sensor = Some((*reading, now)),
            }
        }
    }

    /// The devices that have sent sensor values.
    pub fn sensors(&self) -> Vec<&Device> {
        let mut devices: Vec<&Device> = self.by_address.values().filter(|device| device.sensor.is_some()).collect();
        devices.sort_by_key(|device| device.address);
        devices
    }

    pub fn forget_old(&mut self, now: Duration) {
        self.by_address.retain(|_, device| now.saturating_sub(device.last_seen) < FORGET_AFTER);
    }

    pub fn clear(&mut self) {
        self.by_address.clear();
    }

    /// Strongest signal first.
    pub fn list(&self) -> Vec<&Device> {
        let mut devices: Vec<&Device> = self.by_address.values().collect();
        devices.sort_by(|a, b| b.rssi.cmp(&a.rssi).then(a.address.cmp(&b.address)));
        devices
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn s(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    fn advertisement(last: u8, rssi: i8, name: Option<&str>) -> Advertisement {
        Advertisement { address: [last, 0, 0, 0, 0, 0x11], random: false, rssi, name: name.map(str::to_string), sensor: None }
    }

    #[test]
    fn builds_commands_as_h4_packets() {
        assert_eq!(command(RESET, &[]), vec![0x01, 0x03, 0x0C, 0x00]);
        assert_eq!(command(LE_SET_SCAN_ENABLE, &[1, 0]), vec![0x01, 0x0C, 0x20, 0x02, 0x01, 0x00]);
        let scan = start_scan();
        assert_eq!(scan.first().map(|(opcode, _)| *opcode), Some(RESET));
        let (_, parameters) = &scan[3];
        assert_eq!(parameters, &vec![0x00, 0xE0, 0x01, 0x30, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn parses_events_read_on_the_phone() {
        let mut parser = Parser::default();
        // A reset reply, then the vendor event and reply seen for Read Local Version.
        let bytes = hex("040e0401030c00");
        assert_eq!(parser.push(&bytes), vec![Event::Complete(RESET, 0)]);
        let bytes = hex("04ff09291212172015120900040e0c010110000812090846001212");
        assert_eq!(parser.push(&bytes), vec![Event::Other, Event::Complete(0x1001, 0)]);
    }

    #[test]
    fn joins_events_split_across_reads_and_skips_junk() {
        let mut parser = Parser::default();
        assert_eq!(parser.push(&hex("ff00040e04")), vec![]);
        assert_eq!(parser.push(&hex("010c")), vec![]);
        assert_eq!(parser.push(&hex("2000040f0400010c20")), vec![Event::Complete(LE_SET_SCAN_ENABLE, 0), Event::Complete(LE_SET_SCAN_ENABLE, 0)]);
    }

    #[test]
    fn parses_advertising_reports_with_names() {
        // Two reports: a public address with a complete name, then a random address
        // with only flags. RSSI -65 and -80.
        let event = hex(concat!(
            "043e", "23", "02", "02",
            "00", "00", "665544332211", "0a", "020106", "0609", "53656e73", "72", "bf",
            "00", "01", "aabbccddeeff", "03", "020106", "b0",
        ));
        let mut parser = Parser::default();
        let events = parser.push(&event);
        let Event::Advertisements(reports) = &events[0] else { panic!("{events:?}") };
        assert_eq!(reports.len(), 2);
        assert_eq!(address_text(&reports[0].address), "11:22:33:44:55:66");
        assert!(!reports[0].random);
        assert_eq!(reports[0].rssi, -65);
        assert_eq!(reports[0].name.as_deref(), Some("Sensr"));
        assert!(reports[1].random);
        assert_eq!(reports[1].rssi, -80);
        assert_eq!(reports[1].name, None);
    }

    #[test]
    fn a_truncated_report_is_dropped_not_misread() {
        let mut parser = Parser::default();
        let events = parser.push(&hex("043e0a02010000665544332211"));
        assert_eq!(events, vec![Event::Advertisements(vec![])]);
    }

    #[test]
    fn names_prefer_complete_and_drop_control_characters() {
        assert_eq!(local_name(&hex("0408414243")).as_deref(), Some("ABC"));
        assert_eq!(local_name(&hex("040841424305094c6f6e67")).as_deref(), Some("Long"));
        assert_eq!(local_name(&hex("0409410a42")).as_deref(), Some("AB"));
        assert_eq!(local_name(&hex("0509414243")), None, "length past the end");
        assert_eq!(local_name(&hex("0201060009")), None, "zero length ends the data");
    }

    #[test]
    fn keeps_the_latest_signal_and_a_name_seen_once() {
        let mut devices = Devices::default();
        devices.record(&advertisement(1, -70, Some("Lamp")), s(1));
        devices.record(&advertisement(1, -60, None), s(2));
        devices.record(&advertisement(2, -50, None), s(3));
        let list = devices.list();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].rssi, -50);
        assert_eq!(list[1].name.as_deref(), Some("Lamp"));
        assert_eq!(list[1].rssi, -60);
        assert_eq!(list[1].reports, 2);
        assert_eq!(list[1].last_seen, s(2));
    }

    #[test]
    fn merges_sensor_frames_from_one_device() {
        use crate::sensors::Format;
        let climate = Reading { format: Format::MiBeacon, temperature_centi: Some(2340), humidity_centi: Some(4560), battery_percent: None };
        let battery = Reading { format: Format::MiBeacon, temperature_centi: None, humidity_centi: None, battery_percent: Some(93) };
        let mut devices = Devices::default();
        devices.record(&advertisement(1, -70, None), s(1));
        assert!(devices.sensors().is_empty());
        devices.record(&Advertisement { sensor: Some(climate), ..advertisement(1, -70, None) }, s(2));
        devices.record(&Advertisement { sensor: Some(battery), ..advertisement(1, -71, None) }, s(5));
        devices.record(&advertisement(1, -72, None), s(6));
        let sensors = devices.sensors();
        assert_eq!(sensors.len(), 1);
        let (reading, updated) = sensors[0].sensor.unwrap();
        assert_eq!((reading.temperature_centi, reading.humidity_centi, reading.battery_percent), (Some(2340), Some(4560), Some(93)));
        // Updated by the last sensor frame, not by the plain advertisement after it.
        assert_eq!(updated, s(5));
    }

    #[test]
    fn forgets_quiet_devices_and_stays_under_the_limit() {
        let mut devices = Devices::default();
        devices.record(&advertisement(1, -70, None), s(0));
        devices.record(&advertisement(2, -70, None), s(500));
        devices.forget_old(s(600));
        assert_eq!(devices.list().len(), 1);

        let mut devices = Devices::default();
        for index in 0..DEVICE_LIMIT + 10 {
            let mut address = [0u8; 6];
            address[..2].copy_from_slice(&(index as u16).to_le_bytes());
            devices.record(&Advertisement { address, random: true, rssi: -90, name: None, sensor: None }, s(index as u64));
        }
        assert_eq!(devices.list().len(), DEVICE_LIMIT);
        // The first ones heard made room for the last ones.
        assert!(devices.list().iter().all(|device| device.last_seen >= s(10)));
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap()).collect()
    }
}
