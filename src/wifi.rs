//! Wi-Fi logic that needs no I/O, so it runs on the host in unit tests.
//!
//! `pid1` starts the stock programs. This module parses what they and the kernel
//! report, and checks network details before they reach `wpa_supplicant`.

const SSID_MAX_BYTES: usize = 32;
const PASSPHRASE_BYTES: std::ops::RangeInclusive<usize> = 8..=63;

/// A character device described by a `/sys/dev/char/*/uevent` file.
#[derive(Debug, PartialEq)]
pub struct CharDevice {
    pub name: String,
    pub major: u32,
    pub minor: u32,
}

/// Parses a uevent file. Devices with no name or number are `None`.
pub fn parse_uevent(text: &str) -> Option<CharDevice> {
    let field = |key: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(key)?.strip_prefix('='))
    };
    Some(CharDevice {
        name: field("DEVNAME")?.to_string(),
        major: field("MAJOR")?.parse().ok()?,
        minor: field("MINOR")?.parse().ok()?,
    })
}

/// Network details in the form `wpa_cli set_network` takes.
///
/// `Debug` is left out on purpose, so that the passphrase cannot reach a log
/// through a format string.
pub struct Credentials {
    /// Hex, which `wpa_supplicant` accepts for any SSID without quoting rules.
    pub ssid: String,
    /// The passphrase in double quotes, which is how it is told from a raw key.
    pub psk: String,
}

/// Parses the arguments of `wifi join`: an SSID and a WPA2 passphrase.
pub fn parse_join(text: &str) -> Result<Credentials, &'static str> {
    let words = split_words(text).ok_or("missing closing quote")?;
    let [ssid, passphrase] = words.as_slice() else {
        return Err("usage: wifi join SSID PASSPHRASE");
    };
    credentials(ssid.as_bytes(), passphrase)
}

/// Checks network details. An SSID is bytes, which need not be valid text.
pub fn credentials(ssid: &[u8], passphrase: &str) -> Result<Credentials, &'static str> {
    if ssid.is_empty() || ssid.len() > SSID_MAX_BYTES {
        return Err("the SSID must be 1 to 32 bytes");
    }
    if !PASSPHRASE_BYTES.contains(&passphrase.len())
        || !passphrase.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
    {
        return Err("the passphrase must be 8 to 63 printable ASCII characters");
    }
    Ok(Credentials {
        ssid: ssid.iter().map(|byte| format!("{byte:02x}")).collect(),
        psk: format!("\"{passphrase}\""),
    })
}

/// A network that was found by a scan and that `wifi join` can use.
#[derive(Debug, PartialEq)]
pub struct Network {
    pub ssid: Vec<u8>,
    pub signal: i32,
}

impl Network {
    pub fn label(&self) -> String {
        format!("{:>4} {}", self.signal, display_ssid(&self.ssid))
    }
}

/// An SSID as printable ASCII, which is all the panel's font holds.
pub fn display_ssid(ssid: &[u8]) -> String {
    let text = String::from_utf8_lossy(ssid);
    text.chars().map(|character| if character.is_ascii_graphic() || character == ' ' { character } else { '?' }).collect()
}

/// Parses `wpa_cli scan_results`. It keeps WPA2 personal networks, which are the only
/// ones this system can join, once each, with the strongest first.
pub fn parse_scan_results(text: &str) -> Vec<Network> {
    let mut networks: Vec<Network> = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        let [_, _, signal, flags, ssid] = fields.as_slice() else {
            continue;
        };
        let (Ok(signal), ssid) = (signal.parse::<i32>(), decode_printf(ssid)) else {
            continue;
        };
        if !flags.contains("WPA2-PSK") || ssid.is_empty() {
            continue;
        }
        match networks.iter_mut().find(|network| network.ssid == ssid) {
            Some(known) => known.signal = known.signal.max(signal),
            None => networks.push(Network { ssid, signal }),
        }
    }
    networks.sort_by_key(|network| std::cmp::Reverse(network.signal));
    networks
}

/// Reverses the escaping `wpa_cli` uses for SSIDs: `\\`, `\"`, `\e`, `\n`, `\r`, `\t`,
/// octal and `\xNN`.
fn decode_printf(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;
        if byte != b'\\' || index == bytes.len() {
            decoded.push(byte);
            continue;
        }
        let escape = bytes[index];
        index += 1;
        match escape {
            b'n' => decoded.push(b'\n'),
            b'r' => decoded.push(b'\r'),
            b't' => decoded.push(b'\t'),
            b'e' => decoded.push(0x1b),
            b'x' => {
                let end = (index + 2).min(bytes.len());
                let digits = std::str::from_utf8(&bytes[index..end]).unwrap_or_default();
                let digits = digits.split(|c: char| !c.is_ascii_hexdigit()).next().unwrap_or_default();
                decoded.push(u8::from_str_radix(digits, 16).unwrap_or(0));
                index += digits.len();
            }
            b'0'..=b'7' => {
                let mut value = u32::from(escape - b'0');
                let mut digits = 1;
                while digits < 3 && index < bytes.len() && (b'0'..=b'7').contains(&bytes[index]) {
                    value = value * 8 + u32::from(bytes[index] - b'0');
                    index += 1;
                    digits += 1;
                }
                decoded.push(value as u8);
            }
            other => decoded.push(other),
        }
    }
    decoded
}

/// Reads the level, in dBm, from `/proc/net/wireless`.
pub fn wireless_level(text: &str, interface: &str) -> Option<i32> {
    let line = text.lines().find(|line| line.trim_start().starts_with(&format!("{interface}:")))?;
    line.split_whitespace().nth(3)?.trim_end_matches('.').parse().ok()
}

/// Splits on whitespace. A word that starts with `"` runs to the next `"` that
/// ends a word, so it can hold spaces and other quotes. An unclosed one is `None`.
fn split_words(text: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut rest = text.trim_start();
    while !rest.is_empty() {
        let (word, after) = match rest.strip_prefix('"') {
            Some(quoted) => {
                let end = quoted.match_indices('"').map(|(index, _)| index).find(|&index| {
                    quoted[index + 1..].chars().next().is_none_or(char::is_whitespace)
                })?;
                (&quoted[..end], &quoted[end + 1..])
            }
            None => rest.split_at(rest.find(char::is_whitespace).unwrap_or(rest.len())),
        };
        words.push(word.to_string());
        rest = after.trim_start();
    }
    Some(words)
}

impl Credentials {
    /// The form kept on storage, which `from_saved` reads back.
    pub fn to_saved(&self) -> String {
        format!("{}\n{}\n", self.ssid, self.psk)
    }

    /// The SSID as bytes.
    pub fn ssid_bytes(&self) -> Vec<u8> {
        self.ssid
            .as_bytes()
            .chunks(2)
            .filter_map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
            .collect()
    }

    pub fn from_saved(text: &str) -> Option<Self> {
        let (ssid, psk) = text.strip_suffix('\n')?.split_once('\n')?;
        let valid_ssid = !ssid.is_empty()
            && ssid.len() <= SSID_MAX_BYTES * 2
            && ssid.len() % 2 == 0
            && ssid.bytes().all(|byte| byte.is_ascii_hexdigit());
        let valid_psk = psk.len() >= 2 && psk.starts_with('"') && psk.ends_with('"');
        (valid_ssid && valid_psk)
            .then(|| Self { ssid: ssid.to_string(), psk: psk.to_string() })
    }
}

/// How long until a lease should be renewed: halfway, as DHCP recommends.
pub fn renew_after(lease_seconds: Option<u32>) -> u64 {
    const DEFAULT_LEASE: u64 = 3600;
    const SHORTEST_WAIT: u64 = 60;
    (lease_seconds.map_or(DEFAULT_LEASE, u64::from) / 2).max(SHORTEST_WAIT)
}

/// Length of an Ethernet frame that carries an ARP packet for IPv4.
pub const ARP_FRAME_LEN: usize = 42;
/// The Ethernet type of ARP.
pub const ETHERTYPE_ARP: u16 = 0x0806;

/// Builds a gratuitous ARP request, broadcast, that says `address` is at `mac`.
///
/// After the phone moves to another access point, switches and the old access point
/// can keep sending its frames the old way for minutes. This announcement goes out
/// from the new place and tells them, and every host's ARP cache, where it is now.
/// The request form, with sender and target both set to the address, is the one
/// RFC 5227 uses for announcements.
pub fn gratuitous_arp(mac: [u8; 6], address: [u8; 4]) -> [u8; ARP_FRAME_LEN] {
    let mut frame = [0u8; ARP_FRAME_LEN];
    frame[0..6].copy_from_slice(&[0xff; 6]);
    frame[6..12].copy_from_slice(&mac);
    frame[12..14].copy_from_slice(&ETHERTYPE_ARP.to_be_bytes());
    // Hardware Ethernet, protocol IPv4, address lengths 6 and 4, operation request.
    frame[14..22].copy_from_slice(&[0, 1, 0x08, 0x00, 6, 4, 0, 1]);
    frame[22..28].copy_from_slice(&mac);
    frame[28..32].copy_from_slice(&address);
    // The target hardware address stays zero, as it is unknown in a request.
    frame[38..42].copy_from_slice(&address);
    frame
}

/// Finds `wpa_state` in the output of `wpa_cli status`.
pub fn wpa_state(status: &str) -> Option<&str> {
    status.lines().find_map(|line| line.strip_prefix("wpa_state="))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_uevent_file() {
        assert_eq!(
            parse_uevent("MAJOR=153\nMINOR=0\nDEVNAME=stpwmt\n"),
            Some(CharDevice { name: "stpwmt".into(), major: 153, minor: 0 })
        );
    }

    #[test]
    fn rejects_incomplete_uevent_files() {
        assert_eq!(parse_uevent(""), None);
        assert_eq!(parse_uevent("MAJOR=153\nMINOR=0\n"), None);
        assert_eq!(parse_uevent("MAJOR=x\nMINOR=0\nDEVNAME=a\n"), None);
        // A key that only starts with a wanted name does not count.
        assert_eq!(parse_uevent("MAJORX=1\nMINOR=0\nDEVNAME=a\n"), None);
    }

    fn join(text: &str) -> Result<(String, String), &'static str> {
        parse_join(text).map(|credentials| (credentials.ssid, credentials.psk))
    }

    #[test]
    fn encodes_the_ssid_as_hex_and_quotes_the_passphrase() {
        assert_eq!(join("net password1"), Ok(("6e6574".into(), "\"password1\"".into())));
    }

    #[test]
    fn quoted_words_can_hold_spaces_and_quotes() {
        assert_eq!(join("\"a b\" \"pass word\""), Ok(("612062".into(), "\"pass word\"".into())));
        assert_eq!(join("a \"pa\"ss word\""), Ok(("61".into(), "\"pa\"ss word\"".into())));
        // A quote inside an unquoted word is an ordinary character.
        assert_eq!(join("a pass\"word"), Ok(("61".into(), "\"pass\"word\"".into())));
    }

    #[test]
    fn rejects_bad_join_arguments() {
        assert!(join("").is_err());
        assert!(join("net").is_err());
        assert!(join("net password1 extra").is_err());
        assert_eq!(join("net \"password1"), Err("missing closing quote"));
    }

    #[test]
    fn checks_ssid_and_passphrase_lengths() {
        assert!(join(&format!("{} password1", "s".repeat(32))).is_ok());
        assert!(join(&format!("{} password1", "s".repeat(33))).is_err());
        assert!(join("\"\" password1").is_err());
        assert!(join(&format!("net {}", "p".repeat(8))).is_ok());
        assert!(join(&format!("net {}", "p".repeat(7))).is_err());
        assert!(join(&format!("net {}", "p".repeat(63))).is_ok());
        assert!(join(&format!("net {}", "p".repeat(64))).is_err());
        assert!(join("net pässword1").is_err());
    }

    #[test]
    fn saved_credentials_read_back() {
        let saved = parse_join("net password1").unwrap().to_saved();
        assert_eq!(saved, "6e6574\n\"password1\"\n");
        let read = Credentials::from_saved(&saved).unwrap();
        assert_eq!((read.ssid.as_str(), read.psk.as_str()), ("6e6574", "\"password1\""));
    }

    #[test]
    fn rejects_damaged_saved_credentials() {
        assert!(Credentials::from_saved("").is_none());
        assert!(Credentials::from_saved("6e6574\n").is_none());
        assert!(Credentials::from_saved("6e6574\npassword1\n").is_none());
        assert!(Credentials::from_saved("6e657\n\"password1\"\n").is_none());
        assert!(Credentials::from_saved("zz\n\"password1\"\n").is_none());
        assert!(Credentials::from_saved("6e6574\n\"password1\"").is_none());
    }

    #[test]
    fn scan_results_keep_wpa2_networks_strongest_first() {
        let text = "bssid / frequency / signal level / flags / ssid\n\
            aa:aa\t2412\t-70\t[WPA2-PSK-CCMP][ESS]\tweak\n\
            bb:bb\t2437\t-40\t[WPA-PSK-TKIP+CCMP][WPA2-PSK-TKIP+CCMP][ESS]\tstrong\n\
            cc:cc\t2462\t-30\t[ESS]\topen\n\
            dd:dd\t2412\t-20\t[WPA2-EAP-CCMP][ESS]\tenterprise\n\
            ee:ee\t2412\t-50\t[WPA2-PSK-CCMP][ESS]\tweak\n\
            ff:ff\t2412\t-10\t[WPA2-PSK-CCMP][ESS]\t\n";
        let networks = parse_scan_results(text);
        assert_eq!(
            networks,
            vec![
                Network { ssid: b"strong".to_vec(), signal: -40 },
                Network { ssid: b"weak".to_vec(), signal: -50 },
            ]
        );
    }

    #[test]
    fn decodes_escaped_ssids() {
        assert_eq!(decode_printf(r#"a\\b\"c"#), b"a\\b\"c");
        assert_eq!(decode_printf(r"\xe4\xb8\xad"), "\u{4e2d}".as_bytes());
        assert_eq!(decode_printf(r"\t\n\r\e"), [9, 10, 13, 27]);
        assert_eq!(decode_printf(r"\101x"), b"Ax");
        assert_eq!(decode_printf("end\\"), b"end\\");
        assert_eq!(decode_printf(r"\x"), [0]);
    }

    #[test]
    fn displays_ssids_as_ascii() {
        assert_eq!(display_ssid(b"My Net"), "My Net");
        assert_eq!(display_ssid("\u{4e2d}a".as_bytes()), "?a");
        assert_eq!(display_ssid(&[0xff, b'a']), "?a");
    }

    #[test]
    fn saved_ssid_reads_back_as_bytes() {
        let credentials = credentials(&[0xe4, 0x20], "password1").unwrap();
        assert_eq!(credentials.ssid_bytes(), [0xe4, 0x20]);
    }

    #[test]
    fn reads_the_wireless_level() {
        let text = "Inter-| sta-|   Quality\n face | tus | link level noise\n wlan0: 0000    0.  -35.  -256.       0\n";
        assert_eq!(wireless_level(text, "wlan0"), Some(-35));
        assert_eq!(wireless_level(text, "wlan1"), None);
        assert_eq!(wireless_level("", "wlan0"), None);
    }

    #[test]
    fn renews_halfway_through_a_lease() {
        assert_eq!(renew_after(Some(259_200)), 129_600);
        assert_eq!(renew_after(Some(10)), 60);
        assert_eq!(renew_after(None), 1800);
    }

    #[test]
    fn builds_a_gratuitous_arp_request() {
        let mac = [0x00, 0x08, 0x22, 0x61, 0x75, 0xbe];
        let frame = gratuitous_arp(mac, [192, 168, 1, 104]);
        let expected: [u8; ARP_FRAME_LEN] = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x08, 0x22, 0x61, 0x75, 0xbe, 0x08, 0x06,
            0x00, 0x01, 0x08, 0x00, 6, 4, 0x00, 0x01,
            0x00, 0x08, 0x22, 0x61, 0x75, 0xbe, 192, 168, 1, 104,
            0, 0, 0, 0, 0, 0, 192, 168, 1, 104,
        ];
        assert_eq!(frame, expected);
    }

    #[test]
    fn finds_the_wpa_state() {
        assert_eq!(wpa_state("bssid=x\nwpa_state=COMPLETED\nip_address=y\n"), Some("COMPLETED"));
        assert_eq!(wpa_state("FAILED\n"), None);
    }
}
