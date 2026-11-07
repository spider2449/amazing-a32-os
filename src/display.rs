//! Pixel formats and drawing. Hardware-independent.

/// Pixel layout of one color channel, as reported by the framebuffer driver.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Channel {
    pub offset: u32,
    pub length: u32,
}

impl Channel {
    fn encode(self, value: u8) -> u32 {
        if self.length == 0 {
            return 0;
        }
        (u32::from(value) >> 8u32.saturating_sub(self.length)) << self.offset
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Format {
    pub width: u32,
    pub height: u32,
    /// Bytes per row, which can exceed `width * bytes_per_pixel`.
    pub stride: u32,
    pub bits_per_pixel: u32,
    pub red: Channel,
    pub green: Channel,
    pub blue: Channel,
    pub alpha: Channel,
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("4-byte slice"))
}

fn read_channel(var: &[u8], offset: usize) -> Channel {
    Channel {
        offset: read_u32(var, offset),
        length: read_u32(var, offset + 4),
    }
}

impl Format {
    /// Builds a format from raw `fb_var_screeninfo` and 32-bit ARM
    /// `fb_fix_screeninfo` structures. Rejects layouts that cannot be drawn.
    pub fn from_screeninfo(var: &[u8; VAR_LEN], fix: &[u8; FIX_LEN]) -> Result<Self, String> {
        let format = Self {
            width: read_u32(var, 0),
            height: read_u32(var, 4),
            stride: read_u32(fix, 44),
            bits_per_pixel: read_u32(var, 24),
            red: read_channel(var, 32),
            green: read_channel(var, 44),
            blue: read_channel(var, 56),
            alpha: read_channel(var, 68),
        };
        if !matches!(format.bits_per_pixel, 16 | 24 | 32) {
            return Err(format!("unsupported depth: {format:?}"));
        }
        if format.width == 0
            || format.height == 0
            || format.stride < format.width * format.bytes_per_pixel()
        {
            return Err(format!("unusable geometry: {format:?}"));
        }
        Ok(format)
    }

    fn bytes_per_pixel(&self) -> u32 {
        self.bits_per_pixel / 8
    }

    fn encode(&self, color: Rgb) -> u32 {
        self.red.encode(color.0)
            | self.green.encode(color.1)
            | self.blue.encode(color.2)
            | self.alpha.encode(u8::MAX)
    }
}

pub const VAR_LEN: usize = 160;
pub const FIX_LEN: usize = 68;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub const BACKGROUND: Rgb = Rgb(12, 18, 28);
pub const FOREGROUND: Rgb = Rgb(235, 240, 245);
pub const ACCENT: Rgb = Rgb(240, 150, 60);

/// An area of the screen, in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn contains(self, x: i32, y: i32) -> bool {
        let (left, top) = (self.x as i32, self.y as i32);
        x >= left && y >= top && x < left + self.width as i32 && y < top + self.height as i32
    }

    pub fn inset(self, by: u32) -> Self {
        Self {
            x: self.x + by,
            y: self.y + by,
            width: self.width.saturating_sub(by * 2),
            height: self.height.saturating_sub(by * 2),
        }
    }

    #[cfg(test)]
    pub fn overlaps(self, other: Self) -> bool {
        self.x < other.x + other.width
            && other.x < self.x + self.width
            && self.y < other.y + other.height
            && other.y < self.y + self.height
    }
}

/// An in-memory frame. Drawing outside the frame is clipped.
pub struct Canvas {
    format: Format,
    pixels: Vec<u8>,
}

impl Canvas {
    pub fn new(format: Format) -> Self {
        Self {
            format,
            pixels: vec![0; (format.stride * format.height) as usize],
        }
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn clear(&mut self, color: Rgb) {
        self.fill_rect(0, 0, self.format.width, self.format.height, color);
    }

    pub fn fill(&mut self, area: Rect, color: Rgb) {
        self.fill_rect(area.x, area.y, area.width, area.height, color);
    }

    fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Rgb) {
        let bytes = self.format.bytes_per_pixel() as usize;
        let encoded = self.format.encode(color).to_le_bytes();
        let right = x.saturating_add(width).min(self.format.width) as usize;
        let bottom = y.saturating_add(height).min(self.format.height);
        let (left, stride) = (x as usize, self.format.stride as usize);
        if left >= right || y >= bottom {
            return;
        }
        // Fills one row, then copies it to the others.
        let first = y as usize * stride;
        let span = first + left * bytes..first + right * bytes;
        for pixel in self.pixels[span.clone()].chunks_exact_mut(bytes) {
            pixel.copy_from_slice(&encoded[..bytes]);
        }
        for row in y as usize + 1..bottom as usize {
            let target = row * stride + left * bytes;
            self.pixels.copy_within(span.clone(), target);
        }
    }

    /// Draws text in the middle of an area.
    pub fn draw_text_centered(&mut self, area: Rect, scale: u32, text: &str, color: Rgb) {
        let width = text.chars().count() as u32 * GLYPH_ADVANCE * scale;
        let x = area.x + area.width.saturating_sub(width) / 2;
        let y = area.y + area.height.saturating_sub(7 * scale) / 2;
        self.draw_text(x, y, scale, text, color);
    }

    /// Draws an image given as 8-bit RGB triples, rows top to bottom, clipped to the frame.
    pub fn draw_rgb(&mut self, x: u32, y: u32, width: u32, height: u32, data: &[u8]) {
        let bytes = self.format.bytes_per_pixel() as usize;
        let stride = self.format.stride as usize;
        let columns = width.min(self.format.width.saturating_sub(x)) as usize;
        let rows = height.min(self.format.height.saturating_sub(y)) as usize;
        let format = self.format;
        // Row by row, stopping where `data` runs out.
        for (row, source) in data.chunks(width as usize * 3).take(rows).enumerate() {
            let start = (y as usize + row) * stride + x as usize * bytes;
            let target = &mut self.pixels[start..start + columns * bytes];
            for (pixel, out) in source.chunks_exact(3).take(columns).zip(target.chunks_exact_mut(bytes)) {
                let encoded = format.encode(Rgb(pixel[0], pixel[1], pixel[2])).to_le_bytes();
                out.copy_from_slice(&encoded[..bytes]);
            }
        }
    }

    pub fn draw_text(&mut self, x: u32, y: u32, scale: u32, text: &str, color: Rgb) {
        for (index, character) in text.chars().enumerate() {
            let left = x + index as u32 * (GLYPH_ADVANCE * scale);
            for (column, bits) in glyph(character).iter().enumerate() {
                for row in 0..7 {
                    if bits >> row & 1 == 1 {
                        let px = left + column as u32 * scale;
                        self.fill_rect(px, y + row * scale, scale, scale, color);
                    }
                }
            }
        }
    }

}

/// Horizontal space per character: five glyph columns plus one blank column.
const GLYPH_ADVANCE: u32 = 6;

/// Returns five columns of a 5x7 glyph, with the top row in bit 0. It covers printable
/// ASCII, which is what a passphrase can hold. Other characters render as a box.
fn glyph(character: char) -> [u8; 5] {
    let code = character as u32;
    if !(0x20..=0x7e).contains(&code) {
        return [0x7F, 0x41, 0x41, 0x41, 0x7F];
    }
    FONT[(code - 0x20) as usize]
}

const FONT: [[u8; 5]; 95] = [
    [0x00, 0x00, 0x00, 0x00, 0x00], // space
    [0x00, 0x00, 0x5F, 0x00, 0x00], // !
    [0x00, 0x07, 0x00, 0x07, 0x00], // "
    [0x14, 0x7F, 0x14, 0x7F, 0x14], // #
    [0x24, 0x2A, 0x7F, 0x2A, 0x12], // $
    [0x23, 0x13, 0x08, 0x64, 0x62], // %
    [0x36, 0x49, 0x56, 0x20, 0x50], // &
    [0x00, 0x08, 0x07, 0x03, 0x00], // '
    [0x00, 0x1C, 0x22, 0x41, 0x00], // (
    [0x00, 0x41, 0x22, 0x1C, 0x00], // )
    [0x2A, 0x1C, 0x7F, 0x1C, 0x2A], // *
    [0x08, 0x08, 0x3E, 0x08, 0x08], // +
    [0x00, 0x50, 0x30, 0x00, 0x00], // ,
    [0x08, 0x08, 0x08, 0x08, 0x08], // -
    [0x00, 0x60, 0x60, 0x00, 0x00], // .
    [0x20, 0x10, 0x08, 0x04, 0x02], // /
    [0x3E, 0x51, 0x49, 0x45, 0x3E], // 0
    [0x00, 0x42, 0x7F, 0x40, 0x00], // 1
    [0x42, 0x61, 0x51, 0x49, 0x46], // 2
    [0x21, 0x41, 0x45, 0x4B, 0x31], // 3
    [0x18, 0x14, 0x12, 0x7F, 0x10], // 4
    [0x27, 0x45, 0x45, 0x45, 0x39], // 5
    [0x3C, 0x4A, 0x49, 0x49, 0x30], // 6
    [0x01, 0x71, 0x09, 0x05, 0x03], // 7
    [0x36, 0x49, 0x49, 0x49, 0x36], // 8
    [0x06, 0x49, 0x49, 0x29, 0x1E], // 9
    [0x00, 0x36, 0x36, 0x00, 0x00], // :
    [0x00, 0x56, 0x36, 0x00, 0x00], // ;
    [0x08, 0x14, 0x22, 0x41, 0x00], // <
    [0x14, 0x14, 0x14, 0x14, 0x14], // =
    [0x00, 0x41, 0x22, 0x14, 0x08], // >
    [0x02, 0x01, 0x51, 0x09, 0x06], // ?
    [0x3E, 0x41, 0x5D, 0x59, 0x4E], // @
    [0x7E, 0x11, 0x11, 0x11, 0x7E], // A
    [0x7F, 0x49, 0x49, 0x49, 0x36], // B
    [0x3E, 0x41, 0x41, 0x41, 0x22], // C
    [0x7F, 0x41, 0x41, 0x22, 0x1C], // D
    [0x7F, 0x49, 0x49, 0x49, 0x41], // E
    [0x7F, 0x09, 0x09, 0x09, 0x01], // F
    [0x3E, 0x41, 0x49, 0x49, 0x7A], // G
    [0x7F, 0x08, 0x08, 0x08, 0x7F], // H
    [0x00, 0x41, 0x7F, 0x41, 0x00], // I
    [0x20, 0x40, 0x41, 0x3F, 0x01], // J
    [0x7F, 0x08, 0x14, 0x22, 0x41], // K
    [0x7F, 0x40, 0x40, 0x40, 0x40], // L
    [0x7F, 0x02, 0x0C, 0x02, 0x7F], // M
    [0x7F, 0x04, 0x08, 0x10, 0x7F], // N
    [0x3E, 0x41, 0x41, 0x41, 0x3E], // O
    [0x7F, 0x09, 0x09, 0x09, 0x06], // P
    [0x3E, 0x41, 0x51, 0x21, 0x5E], // Q
    [0x7F, 0x09, 0x19, 0x29, 0x46], // R
    [0x46, 0x49, 0x49, 0x49, 0x31], // S
    [0x01, 0x01, 0x7F, 0x01, 0x01], // T
    [0x3F, 0x40, 0x40, 0x40, 0x3F], // U
    [0x1F, 0x20, 0x40, 0x20, 0x1F], // V
    [0x3F, 0x40, 0x38, 0x40, 0x3F], // W
    [0x63, 0x14, 0x08, 0x14, 0x63], // X
    [0x07, 0x08, 0x70, 0x08, 0x07], // Y
    [0x61, 0x51, 0x49, 0x45, 0x43], // Z
    [0x00, 0x7F, 0x41, 0x41, 0x00], // [
    [0x02, 0x04, 0x08, 0x10, 0x20], // backslash
    [0x00, 0x41, 0x41, 0x7F, 0x00], // ]
    [0x04, 0x02, 0x01, 0x02, 0x04], // ^
    [0x40, 0x40, 0x40, 0x40, 0x40], // _
    [0x00, 0x01, 0x02, 0x04, 0x00], // `
    [0x20, 0x54, 0x54, 0x54, 0x78], // a
    [0x7F, 0x48, 0x44, 0x44, 0x38], // b
    [0x38, 0x44, 0x44, 0x44, 0x20], // c
    [0x38, 0x44, 0x44, 0x48, 0x7F], // d
    [0x38, 0x54, 0x54, 0x54, 0x18], // e
    [0x08, 0x7E, 0x09, 0x01, 0x02], // f
    [0x0C, 0x52, 0x52, 0x52, 0x3E], // g
    [0x7F, 0x08, 0x04, 0x04, 0x78], // h
    [0x00, 0x44, 0x7D, 0x40, 0x00], // i
    [0x20, 0x40, 0x44, 0x3D, 0x00], // j
    [0x7F, 0x10, 0x28, 0x44, 0x00], // k
    [0x00, 0x41, 0x7F, 0x40, 0x00], // l
    [0x7C, 0x04, 0x18, 0x04, 0x78], // m
    [0x7C, 0x08, 0x04, 0x04, 0x78], // n
    [0x38, 0x44, 0x44, 0x44, 0x38], // o
    [0x7C, 0x14, 0x14, 0x14, 0x08], // p
    [0x08, 0x14, 0x14, 0x18, 0x7C], // q
    [0x7C, 0x08, 0x04, 0x04, 0x08], // r
    [0x48, 0x54, 0x54, 0x54, 0x20], // s
    [0x04, 0x3F, 0x44, 0x40, 0x20], // t
    [0x3C, 0x40, 0x40, 0x20, 0x7C], // u
    [0x1C, 0x20, 0x40, 0x20, 0x1C], // v
    [0x3C, 0x40, 0x30, 0x40, 0x3C], // w
    [0x44, 0x28, 0x10, 0x28, 0x44], // x
    [0x0C, 0x50, 0x50, 0x50, 0x3C], // y
    [0x44, 0x64, 0x54, 0x4C, 0x44], // z
    [0x00, 0x08, 0x36, 0x41, 0x00], // {
    [0x00, 0x00, 0x7F, 0x00, 0x00], // |
    [0x00, 0x41, 0x36, 0x08, 0x00], // }
    [0x08, 0x04, 0x08, 0x10, 0x08], // ~
];

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn rgba8888() -> Format {
        Format {
            width: 480,
            height: 854,
            stride: 1920,
            bits_per_pixel: 32,
            red: Channel { offset: 0, length: 8 },
            green: Channel { offset: 8, length: 8 },
            blue: Channel { offset: 16, length: 8 },
            alpha: Channel { offset: 24, length: 8 },
        }
    }

    fn pixel(canvas: &Canvas, x: u32, y: u32) -> [u8; 4] {
        let at = (y * canvas.format.stride + x * 4) as usize;
        canvas.pixels()[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn draws_images_clipped_to_the_frame_and_their_data() {
        let mut canvas = Canvas::new(rgba8888());
        // 2x2 at the right edge: the second column is off the frame.
        canvas.draw_rgb(479, 10, 2, 2, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        assert_eq!(pixel(&canvas, 479, 10), [1, 2, 3, 255]);
        assert_eq!(pixel(&canvas, 479, 11), [7, 8, 9, 255]);
        assert_eq!(pixel(&canvas, 0, 11), [0, 0, 0, 0], "must not wrap to the next row");
        // At the bottom edge, and with data for only one and a half rows.
        canvas.draw_rgb(5, 853, 2, 2, &[1, 1, 1, 2, 2, 2]);
        assert_eq!(pixel(&canvas, 6, 853), [2, 2, 2, 255]);
        canvas.draw_rgb(20, 20, 2, 2, &[3, 3, 3, 4, 4, 4, 5, 5, 5]);
        assert_eq!(pixel(&canvas, 20, 21), [5, 5, 5, 255]);
        assert_eq!(pixel(&canvas, 21, 21), [0, 0, 0, 0]);
    }

    #[test]
    fn encodes_channels_by_driver_layout() {
        let bgra = Format {
            red: Channel { offset: 16, length: 8 },
            blue: Channel { offset: 0, length: 8 },
            ..rgba8888()
        };
        assert_eq!(rgba8888().encode(Rgb(1, 2, 3)), 0xFF03_0201);
        assert_eq!(bgra.encode(Rgb(1, 2, 3)), 0xFF01_0203);

        let rgb565 = Format {
            bits_per_pixel: 16,
            red: Channel { offset: 11, length: 5 },
            green: Channel { offset: 5, length: 6 },
            blue: Channel { offset: 0, length: 5 },
            alpha: Channel::default(),
            ..rgba8888()
        };
        assert_eq!(rgb565.encode(Rgb(255, 0, 255)), 0xF81F);
    }

    #[test]
    fn fill_clips_to_the_frame_and_honors_stride() {
        let format = Format {
            width: 4,
            height: 2,
            stride: 32,
            ..rgba8888()
        };
        let mut canvas = Canvas::new(format);
        canvas.fill_rect(2, 1, 100, 100, Rgb(9, 8, 7));
        assert_eq!(pixel(&canvas, 3, 1), [9, 8, 7, 255]);
        assert_eq!(pixel(&canvas, 1, 1), [0; 4]);
        // Row padding beyond the visible width stays untouched.
        assert_eq!(pixel(&canvas, 4, 1), [0; 4]);
    }

    #[test]
    fn long_text_is_clipped_without_panicking() {
        let mut canvas = Canvas::new(rgba8888());
        canvas.draw_text(470, 850, 8, "WIDE TEXT PAST THE EDGE", FOREGROUND);
    }

    #[test]
    fn the_font_covers_printable_ascii() {
        assert_eq!(glyph(' '), [0; 5]);
        for code in 0x21u8..=0x7e {
            assert_ne!(glyph(char::from(code)), [0; 5], "{}", char::from(code));
            // Bit 7 would fall outside the 7 rows that are drawn.
            assert!(glyph(char::from(code)).iter().all(|column| column & 0x80 == 0));
        }
        assert_ne!(glyph('a'), glyph('A'));
        assert_ne!(glyph('\u{4e2d}'), [0; 5]);
    }

    #[test]
    fn rect_finds_points_and_overlaps() {
        let area = Rect { x: 10, y: 20, width: 30, height: 40 };
        assert!(area.contains(10, 20) && area.contains(39, 59));
        assert!(!area.contains(40, 20) && !area.contains(10, 60) && !area.contains(-1, -1));
        assert!(area.overlaps(Rect { x: 39, y: 59, width: 5, height: 5 }));
        assert!(!area.overlaps(Rect { x: 40, y: 20, width: 5, height: 5 }));
    }

    #[test]
    fn parses_screeninfo_and_rejects_bad_geometry() {
        let mut var = [0u8; VAR_LEN];
        let mut fix = [0u8; FIX_LEN];
        for (offset, value) in [(0, 480), (4, 854), (24, 32), (32, 16), (36, 8), (44, 8)] {
            var[offset..offset + 4].copy_from_slice(&(value as u32).to_le_bytes());
        }
        fix[44..48].copy_from_slice(&1920u32.to_le_bytes());
        let format = Format::from_screeninfo(&var, &fix).unwrap();
        assert_eq!((format.width, format.height, format.stride), (480, 854, 1920));
        assert_eq!(format.red, Channel { offset: 16, length: 8 });

        fix[44..48].copy_from_slice(&100u32.to_le_bytes());
        assert!(Format::from_screeninfo(&var, &fix).is_err());
    }
}
