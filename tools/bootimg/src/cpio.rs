//! Reader and writer for the "newc" cpio format used by Linux initramfs.

const MAGIC: &[u8; 6] = b"070701";
const HEADER_LEN: usize = 110;
const TRAILER: &str = "TRAILER!!!";

pub const S_IFMT: u32 = 0o170000;
pub const S_IFDIR: u32 = 0o040000;
pub const S_IFREG: u32 = 0o100000;

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub mode: u32,
    pub data: Vec<u8>,
}

impl Entry {
    pub fn dir(name: &str) -> Self {
        Self {
            name: name.into(),
            mode: S_IFDIR | 0o755,
            data: Vec::new(),
        }
    }

    pub fn file(name: &str, perm: u32, data: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            mode: S_IFREG | perm,
            data,
        }
    }

    pub fn is_regular(&self) -> bool {
        self.mode & S_IFMT == S_IFREG
    }
}

fn align4(value: usize) -> usize {
    value.div_ceil(4) * 4
}

fn hex_field(header: &[u8], index: usize) -> Result<u32, String> {
    let start = 6 + index * 8;
    let text = std::str::from_utf8(&header[start..start + 8]).map_err(|e| e.to_string())?;
    u32::from_str_radix(text, 16).map_err(|e| e.to_string())
}

pub fn parse(archive: &[u8]) -> Result<Vec<Entry>, String> {
    let mut entries = Vec::new();
    let mut offset = 0;
    loop {
        let header = archive
            .get(offset..offset + HEADER_LEN)
            .ok_or("truncated cpio header")?;
        if &header[..6] != MAGIC {
            return Err(format!("bad cpio magic at offset {offset}"));
        }
        let mode = hex_field(header, 1)?;
        let file_size = hex_field(header, 6)? as usize;
        let name_size = hex_field(header, 11)? as usize;

        let name_start = offset + HEADER_LEN;
        let name = archive
            .get(name_start..name_start + name_size)
            .ok_or("truncated cpio name")?;
        let name = String::from_utf8_lossy(name)
            .trim_end_matches('\0')
            .to_string();
        if name == TRAILER {
            return Ok(entries);
        }

        let data_start = align4(name_start + name_size);
        let data = archive
            .get(data_start..data_start + file_size)
            .ok_or("truncated cpio data")?
            .to_vec();
        offset = align4(data_start + file_size);
        entries.push(Entry { name, mode, data });
    }
}

fn write_entry(out: &mut Vec<u8>, ino: u32, name: &str, mode: u32, data: &[u8]) {
    let name_size = name.len() + 1;
    // Fields: ino, mode, uid, gid, nlink, mtime, filesize, devmajor, devminor,
    // rdevmajor, rdevminor, namesize, check.
    let nlink = if mode & S_IFMT == S_IFDIR { 2 } else { 1 };
    let fields = [
        ino,
        mode,
        0,
        0,
        nlink,
        0,
        data.len() as u32,
        0,
        0,
        0,
        0,
        name_size as u32,
        0,
    ];
    out.extend_from_slice(MAGIC);
    for field in fields {
        out.extend_from_slice(format!("{field:08x}").as_bytes());
    }
    out.extend_from_slice(name.as_bytes());
    out.push(0);
    out.resize(align4(out.len()), 0);
    out.extend_from_slice(data);
    out.resize(align4(out.len()), 0);
}

pub fn build(entries: &[Entry]) -> Vec<u8> {
    let mut out = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        write_entry(&mut out, index as u32 + 1, &entry.name, entry.mode, &entry.data);
    }
    write_entry(&mut out, 0, TRAILER, 0, &[]);
    out.resize(out.len().div_ceil(512) * 512, 0);
    out
}

/// Returns false for names that could escape the extraction directory or that
/// are invalid on Windows. Archive contents are treated as untrusted.
pub fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('/')
        && !name.contains(['\\', ':', '*', '?', '"', '<', '>', '|'])
        && name.split('/').all(|part| part != ".." && part != ".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_entries() {
        let entries = vec![
            Entry::dir("dev"),
            Entry::file("init", 0o755, vec![1, 2, 3, 4, 5]),
            Entry::file("a/b", 0o644, Vec::new()),
        ];
        let archive = build(&entries);
        assert_eq!(archive.len() % 512, 0);
        assert_eq!(parse(&archive).unwrap(), entries);
    }

    #[test]
    fn rejects_corrupt_archives() {
        assert!(parse(b"garbage").is_err());
        let archive = build(&[Entry::file("init", 0o755, vec![0; 100])]);
        assert!(parse(&archive[..150]).is_err());
    }

    #[test]
    fn unsafe_names_are_refused() {
        assert!(is_safe_name("init.rc"));
        assert!(is_safe_name("sbin/adbd"));
        for name in ["", "/etc/passwd", "../x", "a/../../x", "./x", "a\\b", "c:x"] {
            assert!(!is_safe_name(name), "{name}");
        }
    }
}
