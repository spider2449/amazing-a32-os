//! Says what kind of device a BLE advertisement comes from, from the clues it carries:
//! the manufacturer ID, the services it names, and its declared appearance. Phones
//! and tags change their addresses, so these clues are often all there is. Only
//! well known values are listed, and anything else stays unknown rather than being
//! guessed. Nothing here does I/O, so it runs on the host in unit tests.

const AD_UUID16_INCOMPLETE: u8 = 0x02;
const AD_UUID16_COMPLETE: u8 = 0x03;
const AD_SERVICE_DATA_16: u8 = 0x16;
const AD_APPEARANCE: u8 = 0x19;
const AD_MANUFACTURER: u8 = 0xFF;

const APPLE: u16 = 0x004C;

/// A short description, such as `Apple Find My`, or `None` without a known clue.
/// The most specific clue wins: a service or an Apple message type says more than a
/// manufacturer alone.
pub fn kind(mut data: &[u8]) -> Option<String> {
    let mut service = None;
    let mut maker = None;
    let mut appearance = None;
    while let Some((&length, rest)) = data.split_first() {
        let length = usize::from(length);
        if length == 0 || rest.len() < length {
            break;
        }
        let (code, value) = (rest[0], &rest[1..length]);
        match code {
            AD_UUID16_INCOMPLETE | AD_UUID16_COMPLETE => {
                for pair in value.as_chunks::<2>().0 {
                    service = service.or_else(|| service_name(u16::from_le_bytes([pair[0], pair[1]])));
                }
            }
            AD_SERVICE_DATA_16 if value.len() >= 2 => {
                service = service.or_else(|| service_name(u16::from_le_bytes([value[0], value[1]])));
            }
            AD_APPEARANCE if value.len() >= 2 => {
                appearance = appearance_name(u16::from_le_bytes([value[0], value[1]]));
            }
            AD_MANUFACTURER if value.len() >= 2 => {
                let company = u16::from_le_bytes([value[0], value[1]]);
                maker = if company == APPLE { Some(apple_kind(&value[2..])) } else { company_name(company).map(str::to_string) };
            }
            _ => {}
        }
        data = &rest[length..];
    }
    service.map(str::to_string).or(maker).or(appearance.map(str::to_string))
}

/// Apple's manufacturer data starts with a message type, which says what sent it.
fn apple_kind(body: &[u8]) -> String {
    let kind = match body.first() {
        Some(0x02) => "iBeacon",
        Some(0x05) => "Apple AirDrop",
        Some(0x07) => "AirPods or Beats",
        Some(0x09) => "Apple AirPlay",
        Some(0x0C) => "Apple Handoff",
        Some(0x0F) => "Apple Nearby Action",
        Some(0x10) => "iPhone, iPad, Mac or Watch",
        Some(0x12) => "Apple Find My",
        _ => "Apple",
    };
    kind.to_string()
}

fn company_name(company: u16) -> Option<&'static str> {
    Some(match company {
        0x0006 => "Microsoft",
        0x000F => "Broadcom",
        0x0059 => "Nordic",
        0x0075 => "Samsung",
        0x0087 => "Garmin",
        0x00E0 => "Google",
        0x012D => "Sony",
        0x0157 => "Huami (Amazfit, Mi Band)",
        0x0171 => "Amazon",
        0x02E5 => "Espressif",
        0x038F => "Xiaomi",
        0x0499 => "Ruuvi",
        _ => return None,
    })
}

fn service_name(uuid: u16) -> Option<&'static str> {
    Some(match uuid {
        0x180D => "Heart rate monitor",
        0x1812 => "Keyboard, mouse or remote",
        0x181A => "Environment sensor",
        0xFCD2 => "BTHome sensor",
        0xFD5A => "Samsung SmartTag",
        0xFD6F => "Exposure notification",
        0xFE2C => "Google Fast Pair",
        0xFE95 => "Xiaomi",
        0xFEAA => "Eddystone beacon",
        0xFEED => "Tile tracker",
        0xFEF3 => "Google",
        _ => return None,
    })
}

/// The category is the top 10 bits of the appearance value.
fn appearance_name(appearance: u16) -> Option<&'static str> {
    Some(match appearance >> 6 {
        1 => "Phone",
        2 => "Computer",
        3 => "Watch",
        5 => "Display",
        6 => "Remote control",
        12 => "Tag",
        13 => "Thermometer",
        15 => "Keyboard, mouse or remote",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(text: &str) -> Vec<u8> {
        let text: String = text.chars().filter(|character| !character.is_whitespace()).collect();
        (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap()).collect()
    }

    #[test]
    fn names_apple_message_types() {
        // Flags, then manufacturer data: Apple, Nearby Info.
        assert_eq!(kind(&hex("020106 0aff 4c00 1005 0b1c 8e6c 3a")).as_deref(), Some("iPhone, iPad, Mac or Watch"));
        assert_eq!(kind(&hex("05ff 4c00 1219")).as_deref(), Some("Apple Find My"));
        assert_eq!(kind(&hex("05ff 4c00 0719")).as_deref(), Some("AirPods or Beats"));
        assert_eq!(kind(&hex("04ff 4c00 99")).as_deref(), Some("Apple"));
        assert_eq!(kind(&hex("03ff 4c00")).as_deref(), Some("Apple"));
    }

    #[test]
    fn names_companies_and_services() {
        assert_eq!(kind(&hex("05ff 7500 0102")).as_deref(), Some("Samsung"));
        assert_eq!(kind(&hex("05ff 3412 0102")), None, "an unknown company is not guessed");
        // A service is more specific than the company.
        assert_eq!(kind(&hex("0303 2cfe 05ff 0600 0102")).as_deref(), Some("Google Fast Pair"));
        assert_eq!(kind(&hex("0516 95fe 5020")).as_deref(), Some("Xiaomi"));
        // The first known UUID in a list wins.
        assert_eq!(kind(&hex("0502 0d18 1218")).as_deref(), Some("Heart rate monitor"));
        assert_eq!(kind(&hex("0502 3412 1218")).as_deref(), Some("Keyboard, mouse or remote"));
    }

    #[test]
    fn falls_back_to_the_appearance() {
        // 0x00C1: category 3, a watch.
        assert_eq!(kind(&hex("0319c100")).as_deref(), Some("Watch"));
        // A known company beats the appearance.
        assert_eq!(kind(&hex("0319c100 05ff 7500 0102")).as_deref(), Some("Samsung"));
    }

    #[test]
    fn nothing_known_or_broken_data_is_none() {
        assert_eq!(kind(&hex("020106")), None);
        assert_eq!(kind(&hex("09ff 4c00")), None, "a length past the end stops parsing");
        assert_eq!(kind(&[]), None);
    }
}
