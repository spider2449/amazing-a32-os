//! A minimal DHCP client: gets one lease and does not renew it.
//!
//! The message handling has no I/O, so it runs on the host in unit tests.

use std::net::Ipv4Addr;

const OP_REQUEST: u8 = 1;
const OP_REPLY: u8 = 2;
const HTYPE_ETHERNET: u8 = 1;
const MAC_LEN: u8 = 6;
// Ask for broadcast replies, because the interface has no address to receive on.
const FLAG_BROADCAST: u16 = 0x8000;
const COOKIE: [u8; 4] = [99, 130, 83, 99];
const OPTIONS_START: usize = 240;

const OPTION_PAD: u8 = 0;
const OPTION_NETMASK: u8 = 1;
const OPTION_ROUTER: u8 = 3;
const OPTION_DNS: u8 = 6;
const OPTION_REQUESTED_IP: u8 = 50;
const OPTION_LEASE: u8 = 51;
const OPTION_MESSAGE_TYPE: u8 = 53;
const OPTION_SERVER_ID: u8 = 54;
const OPTION_PARAMETER_LIST: u8 = 55;
const OPTION_END: u8 = 255;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MessageType {
    Discover = 1,
    Offer = 2,
    Request = 3,
    Ack = 5,
    Nak = 6,
}

#[derive(Debug, PartialEq)]
pub struct Reply {
    pub kind: MessageType,
    pub address: Ipv4Addr,
    pub server: Option<Ipv4Addr>,
    pub netmask: Option<Ipv4Addr>,
    pub router: Option<Ipv4Addr>,
    pub dns: Option<Ipv4Addr>,
    pub lease_seconds: Option<u32>,
}

/// Builds a Discover, or a Request when `selected` holds the offered address and
/// the server that offered it.
pub fn build_message(xid: u32, mac: [u8; 6], selected: Option<(Ipv4Addr, Ipv4Addr)>) -> Vec<u8> {
    let mut message = vec![0u8; OPTIONS_START];
    message[0] = OP_REQUEST;
    message[1] = HTYPE_ETHERNET;
    message[2] = MAC_LEN;
    message[4..8].copy_from_slice(&xid.to_be_bytes());
    message[10..12].copy_from_slice(&FLAG_BROADCAST.to_be_bytes());
    message[28..34].copy_from_slice(&mac);
    message[236..240].copy_from_slice(&COOKIE);

    let kind = if selected.is_some() { MessageType::Request } else { MessageType::Discover };
    message.extend([OPTION_MESSAGE_TYPE, 1, kind as u8]);
    if let Some((address, server)) = selected {
        message.extend([OPTION_REQUESTED_IP, 4]);
        message.extend(address.octets());
        message.extend([OPTION_SERVER_ID, 4]);
        message.extend(server.octets());
    }
    message.extend([OPTION_PARAMETER_LIST, 3, OPTION_NETMASK, OPTION_ROUTER, OPTION_DNS]);
    message.push(OPTION_END);
    message
}

/// Parses a reply to the given transaction. Anything else is `None`.
pub fn parse_reply(bytes: &[u8], xid: u32, mac: [u8; 6]) -> Option<Reply> {
    if bytes.len() < OPTIONS_START
        || bytes[0] != OP_REPLY
        || bytes[4..8] != xid.to_be_bytes()
        || bytes[28..34] != mac
        || bytes[236..240] != COOKIE
    {
        return None;
    }

    let mut kind = None;
    let mut reply = Reply {
        kind: MessageType::Nak,
        address: ipv4(&bytes[16..20])?,
        server: None,
        netmask: None,
        router: None,
        dns: None,
        lease_seconds: None,
    };

    let mut options = &bytes[OPTIONS_START..];
    while let Some((&code, rest)) = options.split_first() {
        match code {
            OPTION_PAD => {
                options = rest;
                continue;
            }
            OPTION_END => break,
            _ => {}
        }
        let (&length, rest) = rest.split_first()?;
        let length = usize::from(length);
        if rest.len() < length {
            return None;
        }
        let (value, rest) = rest.split_at(length);
        match code {
            OPTION_MESSAGE_TYPE => {
                kind = match value.first() {
                    Some(2) => Some(MessageType::Offer),
                    Some(5) => Some(MessageType::Ack),
                    Some(6) => Some(MessageType::Nak),
                    _ => None,
                }
            }
            OPTION_SERVER_ID => reply.server = ipv4(value),
            OPTION_NETMASK => reply.netmask = ipv4(value),
            OPTION_ROUTER => reply.router = ipv4(value),
            OPTION_DNS => reply.dns = ipv4(value),
            OPTION_LEASE => {
                reply.lease_seconds = value.first_chunk().map(|raw| u32::from_be_bytes(*raw))
            }
            _ => {}
        }
        options = rest;
    }
    reply.kind = kind?;
    Some(reply)
}

/// Reads the first four bytes. Lists, such as several routers, keep the first.
fn ipv4(value: &[u8]) -> Option<Ipv4Addr> {
    value.first_chunk().map(|raw: &[u8; 4]| Ipv4Addr::from(*raw))
}

/// Parses a sysfs MAC address such as `00:08:22:c2:9d:04`.
pub fn parse_mac(text: &str) -> Option<[u8; 6]> {
    let mut mac = [0u8; 6];
    let mut parts = text.trim().split(':');
    for byte in &mut mac {
        *byte = u8::from_str_radix(parts.next()?, 16).ok()?;
    }
    parts.next().is_none().then_some(mac)
}

/// Converts a netmask to a prefix length. Non-contiguous masks are `None`.
pub fn prefix_length(netmask: Ipv4Addr) -> Option<u32> {
    let bits = u32::from(netmask);
    let length = bits.leading_ones();
    (bits.checked_shl(length).unwrap_or(0) == 0).then_some(length)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAC: [u8; 6] = [0, 8, 0x22, 0xc2, 0x9d, 4];
    const XID: u32 = 0x1234_5678;

    fn reply_bytes(kind: u8, xid: u32, mac: [u8; 6]) -> Vec<u8> {
        let mut bytes = vec![0u8; OPTIONS_START];
        bytes[0] = OP_REPLY;
        bytes[4..8].copy_from_slice(&xid.to_be_bytes());
        bytes[16..20].copy_from_slice(&[192, 168, 43, 50]);
        bytes[28..34].copy_from_slice(&mac);
        bytes[236..240].copy_from_slice(&COOKIE);
        bytes.extend([OPTION_PAD, OPTION_MESSAGE_TYPE, 1, kind]);
        bytes.extend([OPTION_SERVER_ID, 4, 192, 168, 43, 1]);
        bytes.extend([OPTION_NETMASK, 4, 255, 255, 255, 0]);
        bytes.extend([OPTION_ROUTER, 8, 192, 168, 43, 1, 10, 0, 0, 1]);
        bytes.extend([OPTION_DNS, 4, 8, 8, 4, 4]);
        bytes.extend([OPTION_LEASE, 4, 0, 0, 14, 16]);
        bytes.extend([12, 3, b'a', b'b', b'c']);
        bytes.push(OPTION_END);
        bytes
    }

    #[test]
    fn discover_has_the_required_fields() {
        let message = build_message(XID, MAC, None);
        assert_eq!(&message[0..3], &[1, 1, 6]);
        assert_eq!(&message[4..8], &XID.to_be_bytes());
        assert_eq!(&message[10..12], &[0x80, 0]);
        assert_eq!(&message[28..34], &MAC);
        assert_eq!(&message[236..240], &COOKIE);
        assert_eq!(&message[240..243], &[53, 1, 1]);
        assert_eq!(*message.last().unwrap(), OPTION_END);
    }

    #[test]
    fn request_names_the_offer_and_its_server() {
        let address = Ipv4Addr::new(192, 168, 43, 50);
        let server = Ipv4Addr::new(192, 168, 43, 1);
        let message = build_message(XID, MAC, Some((address, server)));
        let options = &message[240..];
        assert_eq!(&options[..3], &[53, 1, 3]);
        assert_eq!(&options[3..9], &[50, 4, 192, 168, 43, 50]);
        assert_eq!(&options[9..15], &[54, 4, 192, 168, 43, 1]);
    }

    #[test]
    fn parses_an_offer_and_skips_unknown_options() {
        assert_eq!(
            parse_reply(&reply_bytes(2, XID, MAC), XID, MAC),
            Some(Reply {
                kind: MessageType::Offer,
                address: Ipv4Addr::new(192, 168, 43, 50),
                server: Some(Ipv4Addr::new(192, 168, 43, 1)),
                netmask: Some(Ipv4Addr::new(255, 255, 255, 0)),
                router: Some(Ipv4Addr::new(192, 168, 43, 1)),
                dns: Some(Ipv4Addr::new(8, 8, 4, 4)),
                lease_seconds: Some(3600),
            })
        );
    }

    #[test]
    fn recognizes_ack_and_nak() {
        let kind = |code| parse_reply(&reply_bytes(code, XID, MAC), XID, MAC).map(|r| r.kind);
        assert_eq!(kind(5), Some(MessageType::Ack));
        assert_eq!(kind(6), Some(MessageType::Nak));
        assert_eq!(kind(1), None);
    }

    #[test]
    fn ignores_replies_for_other_clients() {
        assert_eq!(parse_reply(&reply_bytes(2, XID + 1, MAC), XID, MAC), None);
        assert_eq!(parse_reply(&reply_bytes(2, XID, [9; 6]), XID, MAC), None);
    }

    #[test]
    fn rejects_truncated_input_without_panicking() {
        let bytes = reply_bytes(2, XID, MAC);
        for length in 0..bytes.len() {
            let _ = parse_reply(&bytes[..length], XID, MAC);
        }
        // An option claiming more bytes than remain.
        let mut bad = bytes[..OPTIONS_START].to_vec();
        bad.extend([OPTION_MESSAGE_TYPE, 200, 2]);
        assert_eq!(parse_reply(&bad, XID, MAC), None);
    }

    #[test]
    fn parses_mac_addresses() {
        assert_eq!(parse_mac("00:08:22:c2:9d:04\n"), Some(MAC));
        assert_eq!(parse_mac("00:08:22:c2:9d"), None);
        assert_eq!(parse_mac("00:08:22:c2:9d:04:05"), None);
        assert_eq!(parse_mac("zz:08:22:c2:9d:04"), None);
    }

    #[test]
    fn converts_netmasks() {
        assert_eq!(prefix_length(Ipv4Addr::new(255, 255, 255, 0)), Some(24));
        assert_eq!(prefix_length(Ipv4Addr::new(255, 255, 255, 240)), Some(28));
        assert_eq!(prefix_length(Ipv4Addr::new(255, 255, 255, 255)), Some(32));
        assert_eq!(prefix_length(Ipv4Addr::new(0, 0, 0, 0)), Some(0));
        assert_eq!(prefix_length(Ipv4Addr::new(255, 0, 255, 0)), None);
    }
}
