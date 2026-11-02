//! A read-only Android system property area.
//!
//! Stock Android programs read properties from a file mapped by bionic. There is
//! no property service here, so the values are fixed when the file is built. This
//! is the single-file layout that bionic in Android 7.0 reads when
//! `/dev/__properties__` is a file instead of a directory. Other Android versions
//! may differ, so check them on a new device with the stock `getprop`.
//!
//! The file holds a header followed by a tree. Each dot-separated part of a name
//! is a node. Siblings form a binary tree, ordered by length and then by content.
//! All links are byte offsets from the end of the header, and 0 means no link.

const AREA_SIZE: usize = 128 * 1024;
const HEADER_LEN: usize = 128;
const MAGIC: u32 = 0x504f_5250;
const VERSION: u32 = 0xfc6e_d0ab;
/// The longest value bionic accepts, including the terminating NUL.
const VALUE_MAX: usize = 92;

// Field offsets in a tree node: namelen, prop, left, right, children, then name.
const NODE_LEN: usize = 20;
const NODE_PROP: usize = 4;
const NODE_LEFT: usize = 8;
const NODE_RIGHT: usize = 12;
const NODE_CHILDREN: usize = 16;
const NODE_NAME: usize = 20;

// Layout of a property record: serial, the value, then the full name.
const INFO_VALUE: usize = 4;
const INFO_NAME: usize = INFO_VALUE + VALUE_MAX;

/// Builds the file contents for the given properties.
pub fn build_area(properties: &[(&str, &str)]) -> Result<Vec<u8>, String> {
    let mut area = Area::new();
    for (name, value) in properties {
        area.add(name, value)?;
    }
    Ok(area.finish())
}

struct Area {
    bytes: Vec<u8>,
    used: usize,
}

impl Area {
    fn new() -> Self {
        Self {
            bytes: vec![0; AREA_SIZE],
            // The root node has an empty name and sits at offset 0.
            used: NODE_LEN,
        }
    }

    fn finish(mut self) -> Vec<u8> {
        let used = self.used as u32;
        self.bytes[0..4].copy_from_slice(&used.to_ne_bytes());
        self.bytes[8..12].copy_from_slice(&MAGIC.to_ne_bytes());
        self.bytes[12..16].copy_from_slice(&VERSION.to_ne_bytes());
        self.bytes
    }

    fn read(&self, offset: usize) -> u32 {
        let at = HEADER_LEN + offset;
        u32::from_ne_bytes([
            self.bytes[at],
            self.bytes[at + 1],
            self.bytes[at + 2],
            self.bytes[at + 3],
        ])
    }

    fn write(&mut self, offset: usize, value: u32) {
        let at = HEADER_LEN + offset;
        self.bytes[at..at + 4].copy_from_slice(&value.to_ne_bytes());
    }

    /// Reserves zeroed, 4-byte aligned space and returns its offset.
    fn allocate(&mut self, length: usize) -> Result<usize, String> {
        let aligned = length.next_multiple_of(4);
        if HEADER_LEN + self.used + aligned > AREA_SIZE {
            return Err("property area is full".to_string());
        }
        let offset = self.used;
        self.used += aligned;
        Ok(offset)
    }

    fn add(&mut self, name: &str, value: &str) -> Result<(), String> {
        if value.len() >= VALUE_MAX {
            return Err(format!("value of {name} is too long"));
        }
        let mut node = 0;
        for part in name.split('.') {
            if part.is_empty() {
                return Err(format!("bad property name: {name}"));
            }
            node = self.child(node, part)?;
        }

        let info = self.allocate(INFO_NAME + name.len() + 1)?;
        // The value length is in the top byte of the serial. An even serial marks
        // the record as complete.
        self.write(info, (value.len() as u32) << 24);
        let at = HEADER_LEN + info;
        self.bytes[at + INFO_VALUE..at + INFO_VALUE + value.len()]
            .copy_from_slice(value.as_bytes());
        self.bytes[at + INFO_NAME..at + INFO_NAME + name.len()].copy_from_slice(name.as_bytes());
        self.write(node + NODE_PROP, info as u32);
        Ok(())
    }

    /// Finds or creates the child of `parent` named `part`.
    fn child(&mut self, parent: usize, part: &str) -> Result<usize, String> {
        let mut link = parent + NODE_CHILDREN;
        loop {
            let node = self.read(link) as usize;
            if node == 0 {
                let created = self.create_node(part)?;
                self.write(link, created as u32);
                return Ok(created);
            }
            match compare(part.as_bytes(), self.node_name(node)) {
                std::cmp::Ordering::Equal => return Ok(node),
                std::cmp::Ordering::Less => link = node + NODE_LEFT,
                std::cmp::Ordering::Greater => link = node + NODE_RIGHT,
            }
        }
    }

    fn create_node(&mut self, part: &str) -> Result<usize, String> {
        let node = self.allocate(NODE_LEN + part.len() + 1)?;
        self.write(node, part.len() as u32);
        let at = HEADER_LEN + node + NODE_NAME;
        self.bytes[at..at + part.len()].copy_from_slice(part.as_bytes());
        Ok(node)
    }

    fn node_name(&self, node: usize) -> &[u8] {
        let length = self.read(node) as usize;
        let at = HEADER_LEN + node + NODE_NAME;
        &self.bytes[at..at + length]
    }
}

/// Bionic's sibling order: shorter names first, then by content.
fn compare(one: &[u8], two: &[u8]) -> std::cmp::Ordering {
    one.len().cmp(&two.len()).then_with(|| one.cmp(two))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Follows the same steps as bionic's property lookup.
    fn lookup(bytes: &[u8], name: &str) -> Option<String> {
        let read = |offset: usize| {
            let at = HEADER_LEN + offset;
            u32::from_ne_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
        };
        let mut node = 0;
        for part in name.split('.') {
            let mut current = read(node + NODE_CHILDREN);
            loop {
                if current == 0 {
                    return None;
                }
                let start = HEADER_LEN + current + NODE_NAME;
                let found = &bytes[start..start + read(current)];
                match compare(part.as_bytes(), found) {
                    std::cmp::Ordering::Equal => break,
                    std::cmp::Ordering::Less => current = read(current + NODE_LEFT),
                    std::cmp::Ordering::Greater => current = read(current + NODE_RIGHT),
                }
            }
            node = current;
        }
        let info = read(node + NODE_PROP);
        if info == 0 {
            return None;
        }
        let serial = read(info);
        assert_eq!(serial & 1, 0, "a set low bit means the record is being written");
        let start = HEADER_LEN + info + INFO_VALUE;
        let value = &bytes[start..start + (serial >> 24)];

        let stored = &bytes[HEADER_LEN + info + INFO_NAME..];
        let length = stored.iter().position(|&byte| byte == 0).unwrap();
        assert_eq!(&stored[..length], name.as_bytes());
        Some(String::from_utf8(value.to_vec()).unwrap())
    }

    #[test]
    fn header_has_the_magic_and_version_bionic_checks() {
        let bytes = build_area(&[("a.b", "c")]).unwrap();
        assert_eq!(bytes.len(), AREA_SIZE);
        assert_eq!(bytes[8..12], MAGIC.to_ne_bytes());
        assert_eq!(bytes[12..16], VERSION.to_ne_bytes());
    }

    #[test]
    fn finds_properties_through_shared_and_sibling_nodes() {
        let properties = [
            ("service.wcn.driver.ready", "yes"),
            ("service.wcn.coredump.mode", "0"),
            ("ro.hardware", "mt6735"),
            ("ro.a", "1"),
            ("ro.zz", "2"),
            ("ro.mm", "3"),
            ("service", "top"),
        ];
        let bytes = build_area(&properties).unwrap();
        for (name, value) in properties {
            assert_eq!(lookup(&bytes, name).as_deref(), Some(value), "{name}");
        }
        assert_eq!(lookup(&bytes, "service.wcn"), None);
        assert_eq!(lookup(&bytes, "ro.missing"), None);
    }

    #[test]
    fn used_size_is_aligned_and_inside_the_area() {
        let bytes = build_area(&[("abc.de", "x")]).unwrap();
        let used = u32::from_ne_bytes(bytes[0..4].try_into().unwrap()) as usize;
        assert_eq!(used % 4, 0);
        assert!(used > NODE_LEN && used < AREA_SIZE);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(build_area(&[("a", &"x".repeat(VALUE_MAX))]).is_err());
        assert!(build_area(&[("a", &"x".repeat(VALUE_MAX - 1))]).is_ok());
        assert!(build_area(&[("a..b", "x")]).is_err());
    }

    #[test]
    fn reports_a_full_area_instead_of_overflowing() {
        let names: Vec<String> = (0..5000).map(|index| format!("name{index}")).collect();
        let properties: Vec<(&str, &str)> =
            names.iter().map(|name| (name.as_str(), "value")).collect();
        assert!(build_area(&properties).is_err());
    }
}
