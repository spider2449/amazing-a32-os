//! Reads temperature, humidity and battery from the advertisements of BLE sensors,
//! in formats that need no key: Xiaomi MiBeacon without encryption, BTHome v2
//! without encryption, and the pvvx and ATC1441 custom firmware formats. Encrypted
//! advertisements are skipped. Nothing here does I/O, so it runs on the host.

const AD_SERVICE_DATA_16: u8 = 0x16;
const UUID_MIBEACON: u16 = 0xFE95;
const UUID_BTHOME: u16 = 0xFCD2;
// The Environmental Sensing service, which the pvvx and ATC1441 firmware use.
const UUID_ENVIRONMENT: u16 = 0x181A;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    MiBeacon,
    BtHome,
    Pvvx,
    Atc,
}

impl Format {
    pub fn name(self) -> &'static str {
        match self {
            Format::MiBeacon => "mibeacon",
            Format::BtHome => "bthome",
            Format::Pvvx => "pvvx",
            Format::Atc => "atc1441",
        }
    }
}

/// What one advertisement said. Fields it did not carry are `None`: a MiBeacon
/// advertisement holds either the climate or the battery, not both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub format: Format,
    /// Hundredths of a degree Celsius.
    pub temperature_centi: Option<i32>,
    /// Hundredths of a percent.
    pub humidity_centi: Option<u32>,
    pub battery_percent: Option<u8>,
}

impl Reading {
    fn new(format: Format) -> Self {
        Self { format, temperature_centi: None, humidity_centi: None, battery_percent: None }
    }

    fn is_empty(&self) -> bool {
        self.temperature_centi.is_none() && self.humidity_centi.is_none() && self.battery_percent.is_none()
    }

    /// Keeps earlier values that `newer` does not replace.
    pub fn merge(&mut self, newer: &Reading) {
        self.format = newer.format;
        self.temperature_centi = newer.temperature_centi.or(self.temperature_centi);
        self.humidity_centi = newer.humidity_centi.or(self.humidity_centi);
        self.battery_percent = newer.battery_percent.or(self.battery_percent);
    }
}

/// Formats hundredths as a decimal, such as `-0.50` or `23.45`.
pub fn centi_text(value: i64) -> String {
    let sign = if value < 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", value.abs() / 100, value.abs() % 100)
}

pub const CSV_HEADER: &str = "time,address,format,temperature_c,humidity_percent,battery_percent\n";

/// One row of the sensor history. Missing values are left empty.
pub fn csv_line(time: &str, address: &str, reading: &Reading) -> String {
    let text = |value: Option<String>| value.unwrap_or_default();
    format!(
        "{time},{address},{},{},{},{}\n",
        reading.format.name(),
        text(reading.temperature_centi.map(|value| centi_text(value.into()))),
        text(reading.humidity_centi.map(|value| centi_text(value.into()))),
        text(reading.battery_percent.map(|value| value.to_string())),
    )
}

/// Finds a sensor reading in advertising data.
pub fn parse(mut data: &[u8]) -> Option<Reading> {
    while let Some((&length, rest)) = data.split_first() {
        let length = usize::from(length);
        if length == 0 || rest.len() < length {
            break;
        }
        let (kind, value) = (rest[0], &rest[1..length]);
        if kind == AD_SERVICE_DATA_16 && value.len() >= 2 {
            let uuid = u16::from_le_bytes([value[0], value[1]]);
            let body = &value[2..];
            let reading = match uuid {
                UUID_MIBEACON => mibeacon(body),
                UUID_BTHOME => bthome(body),
                UUID_ENVIRONMENT => custom_firmware(body),
                _ => None,
            };
            if let Some(reading) = reading.filter(|reading| !reading.is_empty()) {
                return Some(reading);
            }
        }
        data = &rest[length..];
    }
    None
}

fn i16_le(bytes: &[u8]) -> i32 {
    i32::from(i16::from_le_bytes([bytes[0], bytes[1]]))
}

fn u16_le(bytes: &[u8]) -> u32 {
    u32::from(u16::from_le_bytes([bytes[0], bytes[1]]))
}

/// Xiaomi MiBeacon: frame control (2), product ID (2), frame counter (1), then the
/// MAC address (6) if flagged, a capability byte if flagged, then objects of type
/// (2), length (1) and data. Values are tenths.
fn mibeacon(body: &[u8]) -> Option<Reading> {
    const ENCRYPTED: u16 = 0x0008;
    const HAS_MAC: u16 = 0x0010;
    const HAS_CAPABILITY: u16 = 0x0020;
    const HAS_OBJECT: u16 = 0x0040;
    // In the capability byte, I/O capability adds two more bytes.
    const CAPABILITY_IO: u8 = 0x20;
    if body.len() < 5 {
        return None;
    }
    let control = u16::from_le_bytes([body[0], body[1]]);
    if control & ENCRYPTED != 0 || control & HAS_OBJECT == 0 {
        return None;
    }
    let mut at = 5;
    if control & HAS_MAC != 0 {
        at += 6;
    }
    if control & HAS_CAPABILITY != 0 {
        let capability = *body.get(at)?;
        at += 1;
        if capability & CAPABILITY_IO != 0 {
            at += 2;
        }
    }
    let mut reading = Reading::new(Format::MiBeacon);
    while body.len() >= at + 3 {
        let object = u16::from_le_bytes([body[at], body[at + 1]]);
        let length = usize::from(body[at + 2]);
        let value = body.get(at + 3..at + 3 + length)?;
        match (object, length) {
            (0x1004, 2) => reading.temperature_centi = Some(i16_le(value) * 10),
            (0x1006, 2) => reading.humidity_centi = Some(u16_le(value) * 10),
            (0x100A, 1) => reading.battery_percent = Some(value[0]),
            (0x100D, 4) => {
                reading.temperature_centi = Some(i16_le(&value[0..2]) * 10);
                reading.humidity_centi = Some(u16_le(&value[2..4]) * 10);
            }
            _ => {}
        }
        at += 3 + length;
    }
    Some(reading)
}

/// BTHome v2: a device information byte, then objects of an ID and data whose size
/// the ID fixes. Parsing stops at an ID whose size is not known here, because the
/// rest cannot then be found.
fn bthome(body: &[u8]) -> Option<Reading> {
    const ENCRYPTED: u8 = 0x01;
    let (&info, mut rest) = body.split_first()?;
    if info & ENCRYPTED != 0 || info >> 5 != 2 {
        return None;
    }
    let mut reading = Reading::new(Format::BtHome);
    while let Some((&id, values)) = rest.split_first() {
        // Only IDs whose sizes are certain. 0x12 to 0x14 sit among the one byte binary
        // sensors but are two bytes.
        let size = match id {
            0x00 | 0x01 | 0x09 | 0x0F | 0x10 | 0x11 | 0x15..=0x2D | 0x2E | 0x2F | 0x3A | 0x46 => 1,
            0x02 | 0x03 | 0x06 | 0x07 | 0x08 | 0x0C | 0x0D | 0x0E | 0x12 | 0x13 | 0x14 | 0x3D | 0x3F
            | 0x40 | 0x41 | 0x43 | 0x44 | 0x45 | 0x47 | 0x48 | 0x51 | 0x52 => 2,
            0x04 | 0x05 | 0x0A | 0x0B | 0x42 | 0x4B => 3,
            0x3E | 0x4C | 0x4D | 0x4E | 0x4F | 0x50 | 0x55 => 4,
            _ => break,
        };
        let Some(value) = values.get(..size) else { break };
        match id {
            0x01 => reading.battery_percent = Some(value[0]),
            0x02 => reading.temperature_centi = Some(i16_le(value)),
            0x03 => reading.humidity_centi = Some(u16_le(value)),
            0x2E => reading.humidity_centi = Some(u32::from(value[0]) * 100),
            0x45 => reading.temperature_centi = Some(i16_le(value) * 10),
            _ => {}
        }
        rest = &values[size..];
    }
    Some(reading)
}

/// The pvvx firmware's own format (15 bytes, little-endian, hundredths), or the
/// ATC1441 format (13 bytes, big-endian, tenths and whole percent).
fn custom_firmware(body: &[u8]) -> Option<Reading> {
    match body.len() {
        15 => Some(Reading {
            format: Format::Pvvx,
            temperature_centi: Some(i16_le(&body[6..8])),
            humidity_centi: Some(u16_le(&body[8..10])),
            battery_percent: Some(body[12]),
        }),
        13 => Some(Reading {
            format: Format::Atc,
            temperature_centi: Some(i32::from(i16::from_be_bytes([body[6], body[7]])) * 10),
            humidity_centi: Some(u32::from(body[8]) * 100),
            battery_percent: Some(body[9]),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        let text: String = text.chars().filter(|character| !character.is_whitespace()).collect();
        (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap()).collect()
    }

    /// Wraps service data in an advertisement: flags, then the service data.
    fn advertisement(uuid: u16, body: &[u8]) -> Vec<u8> {
        let mut data = hex("020106");
        data.push(body.len() as u8 + 3);
        data.push(AD_SERVICE_DATA_16);
        data.extend_from_slice(&uuid.to_le_bytes());
        data.extend_from_slice(body);
        data
    }

    #[test]
    fn reads_mibeacon_climate_and_battery_frames() {
        // Frame control 0x2050 and product 0x01AA, as heard from a LYWSDCGQ on this
        // phone, with a made up counter, address and values: 23.4 C and 45.6%.
        let climate = hex("5020 aa01 17 665544332211 0d10 04 ea00 c801");
        let reading = parse(&advertisement(UUID_MIBEACON, &climate)).unwrap();
        assert_eq!(reading.format, Format::MiBeacon);
        assert_eq!(reading.temperature_centi, Some(2340));
        assert_eq!(reading.humidity_centi, Some(4560));
        assert_eq!(reading.battery_percent, None);

        let battery = hex("5020 aa01 18 665544332211 0a10 01 5d");
        let reading = parse(&advertisement(UUID_MIBEACON, &battery)).unwrap();
        assert_eq!(reading.battery_percent, Some(93));
        assert_eq!(reading.temperature_centi, None);

        // A temperature below zero: -5.2 C.
        let cold = hex("5020 aa01 19 665544332211 0410 02 ccff");
        assert_eq!(parse(&advertisement(UUID_MIBEACON, &cold)).unwrap().temperature_centi, Some(-520));
    }

    #[test]
    fn skips_mibeacon_that_is_encrypted_or_has_no_object() {
        let encrypted = hex("5820 5b05 17 665544332211 0d10 04 ea00 c801");
        assert_eq!(parse(&advertisement(UUID_MIBEACON, &encrypted)), None);
        let no_object = hex("1020 aa01 17 665544332211");
        assert_eq!(parse(&advertisement(UUID_MIBEACON, &no_object)), None);
        // An object longer than the data is not misread.
        let short = hex("5020 aa01 17 665544332211 0d10 04 ea00");
        assert_eq!(parse(&advertisement(UUID_MIBEACON, &short)), None);
    }

    #[test]
    fn reads_mibeacon_with_a_capability_byte() {
        // Capability 0x20 adds two I/O capability bytes before the object.
        let body = hex("7020 aa01 17 665544332211 20 0000 0610 02 c801");
        assert_eq!(parse(&advertisement(UUID_MIBEACON, &body)).unwrap().humidity_centi, Some(4560));
    }

    #[test]
    fn reads_bthome_v2() {
        // The BTHome example: packet ID 0x5c, temperature 25.06 C, humidity 50.55%,
        // battery 93%, after an unencrypted version 2 information byte.
        let body = hex("40 00 5c 02 ca09 03 bf13 01 5d");
        let reading = parse(&advertisement(UUID_BTHOME, &body)).unwrap();
        assert_eq!(reading.format, Format::BtHome);
        assert_eq!(reading.temperature_centi, Some(2506));
        assert_eq!(reading.humidity_centi, Some(5055));
        assert_eq!(reading.battery_percent, Some(93));

        // CO2 (0x12) is two bytes, although nearby IDs are one.
        let body = hex("40 12 e204 02 ca09");
        assert_eq!(parse(&advertisement(UUID_BTHOME, &body)).unwrap().temperature_centi, Some(2506));

        // Tenths of a degree and whole percent, and an unknown ID that ends parsing.
        let body = hex("40 45 1301 2e 37 fe 01 5d");
        let reading = parse(&advertisement(UUID_BTHOME, &body)).unwrap();
        assert_eq!(reading.temperature_centi, Some(2750));
        assert_eq!(reading.humidity_centi, Some(5500));
        assert_eq!(reading.battery_percent, None);
    }

    #[test]
    fn skips_bthome_that_is_encrypted_or_another_version() {
        assert_eq!(parse(&advertisement(UUID_BTHOME, &hex("41 02 ca09"))), None);
        assert_eq!(parse(&advertisement(UUID_BTHOME, &hex("20 02 ca09"))), None);
    }

    #[test]
    fn reads_the_custom_firmware_formats() {
        // pvvx: address (6), 21.55 C, 48.12%, 2950 mV, 85%, counter, flags.
        let pvvx = hex("665544332211 6b08 cc12 860b 55 07 04");
        let reading = parse(&advertisement(UUID_ENVIRONMENT, &pvvx)).unwrap();
        assert_eq!(reading.format, Format::Pvvx);
        assert_eq!(reading.temperature_centi, Some(2155));
        assert_eq!(reading.humidity_centi, Some(4812));
        assert_eq!(reading.battery_percent, Some(85));

        // ATC1441: address (6), 21.5 C big-endian, 48%, 85%, 2950 mV, counter.
        let atc = hex("a4c138112233 00d7 30 55 0b86 07");
        let reading = parse(&advertisement(UUID_ENVIRONMENT, &atc)).unwrap();
        assert_eq!(reading.format, Format::Atc);
        assert_eq!(reading.temperature_centi, Some(2150));
        assert_eq!(reading.humidity_centi, Some(4800));
        assert_eq!(reading.battery_percent, Some(85));

        assert_eq!(parse(&advertisement(UUID_ENVIRONMENT, &hex("0102"))), None);
    }

    #[test]
    fn ignores_other_data_and_bad_lengths() {
        assert_eq!(parse(&hex("020106 0bff4c0010050b1c")), None);
        assert_eq!(parse(&hex("ff16")), None);
        assert_eq!(parse(&[]), None);
    }

    #[test]
    fn merges_partial_readings() {
        let mut reading = Reading { format: Format::MiBeacon, temperature_centi: Some(2340), humidity_centi: Some(4560), battery_percent: None };
        reading.merge(&Reading { format: Format::MiBeacon, temperature_centi: None, humidity_centi: None, battery_percent: Some(93) });
        assert_eq!(reading.temperature_centi, Some(2340));
        assert_eq!(reading.battery_percent, Some(93));
    }

    #[test]
    fn writes_csv_rows() {
        let reading = Reading { format: Format::Atc, temperature_centi: Some(-520), humidity_centi: None, battery_percent: Some(85) };
        assert_eq!(csv_line("2026-11-06T12:00:00Z", "a4:c1:38:11:22:33", &reading), "2026-11-06T12:00:00Z,a4:c1:38:11:22:33,atc1441,-5.20,,85\n");
        assert_eq!(CSV_HEADER.split(',').count(), 6);
    }

    #[test]
    fn formats_hundredths() {
        assert_eq!(centi_text(2345), "23.45");
        assert_eq!(centi_text(-50), "-0.50");
        assert_eq!(centi_text(-520), "-5.20");
        assert_eq!(centi_text(7), "0.07");
    }
}
