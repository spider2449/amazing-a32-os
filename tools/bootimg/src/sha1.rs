//! Minimal SHA-1, needed only for the non-security boot image ID field.

pub struct Sha1 {
    state: [u32; 5],
    buffer: Vec<u8>,
    length: u64,
}

impl Sha1 {
    pub fn new() -> Self {
        Self {
            state: [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0],
            buffer: Vec::with_capacity(64),
            length: 0,
        }
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.length += data.len() as u64;
        if !self.buffer.is_empty() {
            let take = data.len().min(64 - self.buffer.len());
            self.buffer.extend_from_slice(&data[..take]);
            data = &data[take..];
            if self.buffer.len() < 64 {
                return;
            }
            let block = std::mem::take(&mut self.buffer);
            self.compress(&block);
        }
        let mut blocks = data.chunks_exact(64);
        for block in &mut blocks {
            self.compress(block);
        }
        self.buffer.extend_from_slice(blocks.remainder());
    }

    pub fn finish(mut self) -> [u8; 20] {
        let bits = self.length * 8;
        let padding = (119 - (self.length % 64) as usize) % 64;
        self.update(&[0x80]);
        self.update(&vec![0; padding]);
        self.update(&bits.to_be_bytes());

        let mut digest = [0; 20];
        for (out, word) in digest.chunks_exact_mut(4).zip(self.state) {
            out.copy_from_slice(&word.to_be_bytes());
        }
        digest
    }

    fn compress(&mut self, block: &[u8]) {
        let mut w = [0u32; 80];
        for (word, bytes) in w.iter_mut().zip(block.chunks_exact(4)) {
            *word = u32::from_be_bytes(bytes.try_into().expect("4-byte chunk"));
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let [mut a, mut b, mut c, mut d, mut e] = self.state;
        for (i, &word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let next = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(word);
            (e, d, c, b, a) = (d, c, b.rotate_left(30), a, next);
        }
        for (state, value) in self.state.iter_mut().zip([a, b, c, d, e]) {
            *state = state.wrapping_add(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Sha1;

    fn hex(data: &[&[u8]]) -> String {
        let mut sha = Sha1::new();
        for part in data {
            sha.update(part);
        }
        sha.finish().iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn matches_known_vectors() {
        assert_eq!(hex(&[b""]), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(hex(&[b"abc"]), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn chunking_does_not_change_digest() {
        let data = vec![b'a'; 1000];
        assert_eq!(hex(&[&data]), hex(&[&data[..1], &data[1..65], &data[65..]]));
        assert_eq!(hex(&[&data]), "291e9a6c66994949b57ba5e650361e98fc36b1ba");
    }
}
