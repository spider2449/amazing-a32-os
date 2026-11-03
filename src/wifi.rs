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
    if ssid.is_empty() || ssid.len() > SSID_MAX_BYTES {
        return Err("the SSID must be 1 to 32 bytes");
    }
    if !PASSPHRASE_BYTES.contains(&passphrase.len())
        || !passphrase.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
    {
        return Err("the passphrase must be 8 to 63 printable ASCII characters");
    }
    Ok(Credentials {
        ssid: ssid.bytes().map(|byte| format!("{byte:02x}")).collect(),
        psk: format!("\"{passphrase}\""),
    })
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
    fn finds_the_wpa_state() {
        assert_eq!(wpa_state("bssid=x\nwpa_state=COMPLETED\nip_address=y\n"), Some("COMPLETED"));
        assert_eq!(wpa_state("FAILED\n"), None);
    }
}
