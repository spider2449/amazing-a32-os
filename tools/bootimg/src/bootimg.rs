//! Android boot image (header version 0) parsing and assembly.

use crate::sha1::Sha1;

pub const MAGIC: &[u8; 8] = b"ANDROID!";
const ID_OFFSET: usize = 576;
const ID_LEN: usize = 32;
const HEADER_LEN: usize = 1632;

/// A parsed boot image. `header` keeps the original fields so that unknown
/// values (addresses, OS version, command line) survive a rebuild untouched.
#[derive(Debug, Clone)]
pub struct BootImage {
    pub header: Vec<u8>,
    pub kernel: Vec<u8>,
    pub ramdisk: Vec<u8>,
    pub second: Vec<u8>,
    /// Bytes after the last section, such as a verified boot signature.
    pub trailer: Vec<u8>,
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("4-byte slice"))
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn aligned(size: usize, page: usize) -> usize {
    size.div_ceil(page) * page
}

impl BootImage {
    pub fn parse(image: &[u8]) -> Result<Self, String> {
        if image.len() < HEADER_LEN || &image[..8] != MAGIC {
            return Err("not an Android boot image".into());
        }
        let page = read_u32(image, 36) as usize;
        if !page.is_power_of_two() || page < HEADER_LEN {
            return Err(format!("unsupported page size {page}"));
        }
        if read_u32(image, 40) != 0 {
            return Err("images with a dt section are not supported".into());
        }

        let mut offset = page;
        let mut section = |size: usize| -> Result<Vec<u8>, String> {
            let end = offset.checked_add(size).filter(|&end| end <= image.len());
            let end = end.ok_or("section extends past the end of the image")?;
            let data = image[offset..end].to_vec();
            offset = aligned(end, page).min(image.len());
            Ok(data)
        };
        let kernel = section(read_u32(image, 8) as usize)?;
        let ramdisk = section(read_u32(image, 16) as usize)?;
        let second = section(read_u32(image, 24) as usize)?;

        Ok(Self {
            header: image[..HEADER_LEN].to_vec(),
            kernel,
            ramdisk,
            second,
            trailer: image[offset..].to_vec(),
        })
    }

    pub fn page_size(&self) -> usize {
        read_u32(&self.header, 36) as usize
    }

    /// Text field of up to `len` bytes at `offset`, cut at the first NUL.
    pub fn text(&self, offset: usize, len: usize) -> String {
        let field = &self.header[offset..offset + len];
        let end = field.iter().position(|&b| b == 0).unwrap_or(len);
        String::from_utf8_lossy(&field[..end]).into_owned()
    }

    pub fn field(&self, offset: usize) -> u32 {
        read_u32(&self.header, offset)
    }

    /// The image ID defined by mkbootimg: SHA-1 over each section and its size.
    pub fn computed_id(&self) -> [u8; ID_LEN] {
        let mut sha = Sha1::new();
        for section in [&self.kernel, &self.ramdisk, &self.second] {
            sha.update(section);
            sha.update(&(section.len() as u32).to_le_bytes());
        }
        let mut id = [0; ID_LEN];
        id[..20].copy_from_slice(&sha.finish());
        id
    }

    pub fn id_matches(&self) -> bool {
        self.header[ID_OFFSET..ID_OFFSET + ID_LEN] == self.computed_id()
    }

    /// Swaps the ramdisk and drops the trailer, whose signature would no
    /// longer match the new contents.
    pub fn with_ramdisk(&self, ramdisk: Vec<u8>) -> Self {
        Self {
            ramdisk,
            trailer: Vec::new(),
            ..self.clone()
        }
    }

    /// Swaps both the kernel and ramdisk and drops the trailer, whose signature
    /// would no longer match the new contents. Header addresses and command line
    /// remain those of the template image.
    pub fn with_kernel_and_ramdisk(&self, kernel: Vec<u8>, ramdisk: Vec<u8>) -> Self {
        Self {
            kernel,
            ramdisk,
            trailer: Vec::new(),
            ..self.clone()
        }
    }

    /// Serializes the image with refreshed section sizes and ID.
    pub fn build(&self) -> Vec<u8> {
        let page = self.page_size();
        let mut header = self.header.clone();
        write_u32(&mut header, 8, self.kernel.len() as u32);
        write_u32(&mut header, 16, self.ramdisk.len() as u32);
        write_u32(&mut header, 24, self.second.len() as u32);
        header[ID_OFFSET..ID_OFFSET + ID_LEN].copy_from_slice(&self.computed_id());

        let mut image = Vec::new();
        for section in [&header, &self.kernel, &self.ramdisk, &self.second] {
            image.extend_from_slice(section);
            image.resize(aligned(image.len(), page), 0);
        }
        image.extend_from_slice(&self.trailer);
        image
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> BootImage {
        let mut header = vec![0; HEADER_LEN];
        header[..8].copy_from_slice(MAGIC);
        write_u32(&mut header, 36, 2048);
        header[64..67].copy_from_slice(b"abc");
        BootImage {
            header,
            kernel: vec![1; 3000],
            ramdisk: vec![2; 100],
            second: Vec::new(),
            trailer: vec![9; 300],
        }
    }

    #[test]
    fn round_trip_is_byte_identical() {
        let image = sample().build();
        assert_eq!(image.len(), 2048 + 4096 + 2048 + 300);
        let parsed = BootImage::parse(&image).unwrap();
        assert!(parsed.id_matches());
        assert_eq!(parsed.text(64, 512), "abc");
        assert_eq!(parsed.trailer, vec![9; 300]);
        assert_eq!(parsed.build(), image);
    }

    #[test]
    fn replacing_ramdisk_updates_id_and_drops_trailer() {
        let original = BootImage::parse(&sample().build()).unwrap();
        let parsed = BootImage::parse(&original.with_ramdisk(vec![3; 5000]).build()).unwrap();
        assert!(parsed.id_matches());
        assert_eq!(parsed.ramdisk, vec![3; 5000]);
        assert!(parsed.trailer.is_empty());
        assert_eq!(parsed.kernel, original.kernel);
    }

    #[test]
    fn replacing_kernel_and_ramdisk_updates_id_and_keeps_header_fields() {
        let original = BootImage::parse(&sample().build()).unwrap();
        let parsed = BootImage::parse(
            &original
                .with_kernel_and_ramdisk(vec![4; 4097], vec![5; 257])
                .build(),
        )
        .unwrap();
        assert!(parsed.id_matches());
        assert_eq!(parsed.kernel, vec![4; 4097]);
        assert_eq!(parsed.ramdisk, vec![5; 257]);
        assert!(parsed.trailer.is_empty());
        assert_eq!(parsed.page_size(), original.page_size());
        assert_eq!(parsed.text(64, 512), original.text(64, 512));
        assert_eq!(parsed.field(12), original.field(12));
        assert_eq!(parsed.field(20), original.field(20));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(BootImage::parse(b"short").is_err());
        let mut truncated = sample().build();
        truncated.truncate(3000);
        assert!(BootImage::parse(&truncated).is_err());
    }
}
