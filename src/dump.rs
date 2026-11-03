//! Text encoding for copying file ranges over the serial console.
//!
//! The console is a line-oriented terminal, so binary data goes out as base64
//! with a CRC-32 that lets the host detect damage and request the range again.
//!
//! Output format, one item per line:
//! `DUMP <bytes read> <crc32 in hex>`, the base64 data, then `END`.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};

/// Largest range served by one request, which bounds the memory used.
pub const MAX_LENGTH: u64 = 256 * 1024;

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Writes up to `length` bytes of `path` from `offset`. The file is only
/// opened for reading. A range past the end of the file yields fewer bytes.
pub fn dump(path: &str, offset: u64, length: u64, out: &mut impl Write) -> io::Result<()> {
    if length > MAX_LENGTH {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("length is over {MAX_LENGTH}"),
        ));
    }
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut data = Vec::with_capacity(length as usize);
    file.take(length).read_to_end(&mut data)?;

    writeln!(out, "DUMP {} {:08x}", data.len(), crc32(&data))?;
    out.write_all(&base64(&data))?;
    out.write_all(b"\nEND\n")
}

/// Returns the partition name from a block device's sysfs `uevent` text.
pub fn partition_name(uevent: &str) -> Option<&str> {
    uevent.lines().find_map(|line| line.strip_prefix("PARTNAME="))
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn base64(data: &[u8]) -> Vec<u8> {
    let mut text = Vec::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let mut group = [0u8; 3];
        group[..chunk.len()].copy_from_slice(chunk);
        let bits = u32::from_be_bytes([0, group[0], group[1], group[2]]);
        for position in 0..4 {
            if position <= chunk.len() {
                let index = (bits >> (18 - 6 * position)) & 63;
                text.push(BASE64[index as usize]);
            } else {
                text.push(b'=');
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_the_partition_name() {
        assert_eq!(partition_name("MAJOR=179\nPARTN=7\nPARTNAME=boot\n"), Some("boot"));
        assert_eq!(partition_name("MAJOR=179\nDEVTYPE=disk\n"), None);
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        let encode = |data: &[u8]| String::from_utf8(base64(data)).unwrap();
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"f"), "Zg==");
        assert_eq!(encode(b"fo"), "Zm8=");
        assert_eq!(encode(b"foo"), "Zm9v");
        assert_eq!(encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn dumps_a_range_and_stops_at_end_of_file() {
        let path = std::env::temp_dir().join(format!("a32-dump-test-{}", std::process::id()));
        fs::write(&path, b"xxfoobar").unwrap();
        let path_text = path.to_str().unwrap();

        let mut out = Vec::new();
        dump(path_text, 2, 100, &mut out).unwrap();
        let expected = format!("DUMP 6 {:08x}\nZm9vYmFy\nEND\n", crc32(b"foobar"));
        assert_eq!(String::from_utf8(out).unwrap(), expected);

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn rejects_oversized_requests() {
        let err = dump("unused", 0, MAX_LENGTH + 1, &mut Vec::new()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }
}
