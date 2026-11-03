//! A minimal SNTP client's messages. The handling has no I/O, so it runs on the
//! host in unit tests.

pub const MESSAGE_LEN: usize = 48;

const VERSION_4_CLIENT: u8 = (4 << 3) | 3;
const MODE_MASK: u8 = 0x07;
const MODE_SERVER: u8 = 4;
const LEAP_MASK: u8 = 0xc0;
const LEAP_UNSYNCHRONIZED: u8 = 0xc0;
const ORIGIN_OFFSET: usize = 24;
const TRANSMIT_OFFSET: usize = 40;
// Seconds from the NTP epoch, 1900, to the Unix epoch, 1970.
const UNIX_OFFSET: u64 = 2_208_988_800;
// The clock takes only times in this range: from 2024, which rejects a server
// with a wrong clock, to the last second a 32-bit time can hold on this kernel.
const EARLIEST: u64 = 1_704_067_200;
const LATEST: u64 = i32::MAX as u64;

/// Builds a request. A server copies `nonce` into its reply, which ties the reply
/// to this request.
pub fn build_request(nonce: u64) -> [u8; MESSAGE_LEN] {
    let mut message = [0u8; MESSAGE_LEN];
    message[0] = VERSION_4_CLIENT;
    message[TRANSMIT_OFFSET..].copy_from_slice(&nonce.to_be_bytes());
    message
}

/// Returns the Unix time in a valid reply to the request with this nonce.
pub fn parse_reply(bytes: &[u8], nonce: u64) -> Option<u64> {
    if bytes.len() < MESSAGE_LEN
        || bytes[0] & MODE_MASK != MODE_SERVER
        || bytes[0] & LEAP_MASK == LEAP_UNSYNCHRONIZED
        // Stratum 0 means the server refuses, and over 15 means it is unsynchronized.
        || !(1..=15).contains(&bytes[1])
        || bytes[ORIGIN_OFFSET..ORIGIN_OFFSET + 8] != nonce.to_be_bytes()
    {
        return None;
    }
    let seconds: [u8; 4] = bytes[TRANSMIT_OFFSET..TRANSMIT_OFFSET + 4].try_into().ok()?;
    let ntp = u64::from(u32::from_be_bytes(seconds));
    // The 32-bit field wraps in 2036. Small values are past that, because the
    // range below rejects anything before 2024 anyway.
    let unix = if ntp < UNIX_OFFSET { ntp + (1 << 32) - UNIX_OFFSET } else { ntp - UNIX_OFFSET };
    (EARLIEST..=LATEST).contains(&unix).then_some(unix)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONCE: u64 = 0x0102_0304_0506_0708;
    const TIME: u64 = 1_793_000_000;

    fn reply(unix: u64) -> [u8; MESSAGE_LEN] {
        let mut bytes = [0u8; MESSAGE_LEN];
        bytes[0] = (4 << 3) | 4;
        bytes[1] = 2;
        bytes[ORIGIN_OFFSET..ORIGIN_OFFSET + 8].copy_from_slice(&NONCE.to_be_bytes());
        let ntp = (unix + UNIX_OFFSET) as u32;
        bytes[TRANSMIT_OFFSET..TRANSMIT_OFFSET + 4].copy_from_slice(&ntp.to_be_bytes());
        bytes
    }

    #[test]
    fn request_is_a_version_4_client_message() {
        let request = build_request(NONCE);
        assert_eq!(request[0], 0x23);
        assert_eq!(request[40..48], NONCE.to_be_bytes());
    }

    #[test]
    fn reads_the_time_from_a_reply() {
        assert_eq!(parse_reply(&reply(TIME), NONCE), Some(TIME));
    }

    #[test]
    fn rejects_replies_that_do_not_match() {
        assert_eq!(parse_reply(&reply(TIME), NONCE + 1), None);
        assert_eq!(parse_reply(&reply(TIME)[..47], NONCE), None);

        let mut client_mode = reply(TIME);
        client_mode[0] = 0x23;
        assert_eq!(parse_reply(&client_mode, NONCE), None);

        let mut refused = reply(TIME);
        refused[1] = 0;
        assert_eq!(parse_reply(&refused, NONCE), None);

        let mut unsynchronized = reply(TIME);
        unsynchronized[0] |= LEAP_MASK;
        assert_eq!(parse_reply(&unsynchronized, NONCE), None);
    }

    #[test]
    fn rejects_times_the_clock_should_not_take() {
        assert_eq!(parse_reply(&reply(EARLIEST - 1), NONCE), None);
        assert_eq!(parse_reply(&reply(LATEST + 1), NONCE), None);
        assert_eq!(parse_reply(&reply(LATEST), NONCE), Some(LATEST));
    }

    #[test]
    fn reads_times_after_the_field_wraps_in_2036() {
        const AFTER_WRAP: u64 = 2_100_000_000;
        assert_eq!(parse_reply(&reply(AFTER_WRAP), NONCE), Some(AFTER_WRAP));
    }
}
