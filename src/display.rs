//! Pixel formats, drawing and the status screen. Hardware-independent.

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

const BACKGROUND: Rgb = Rgb(12, 18, 28);
const FOREGROUND: Rgb = Rgb(235, 240, 245);
const ACCENT: Rgb = Rgb(240, 150, 60);
const RED: Rgb = Rgb(255, 0, 0);
const GREEN: Rgb = Rgb(0, 255, 0);
const BLUE: Rgb = Rgb(0, 0, 255);

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

    fn fill_rect(&mut self, x: u32, y: u32, width: u32, height: u32, color: Rgb) {
        let bytes = self.format.bytes_per_pixel() as usize;
        let encoded = self.format.encode(color).to_le_bytes();
        let right = x.saturating_add(width).min(self.format.width) as usize;
        let bottom = y.saturating_add(height).min(self.format.height);
        for row in y..bottom {
            let start = (row * self.format.stride) as usize;
            for column in x as usize..right {
                let at = start + column * bytes;
                self.pixels[at..at + bytes].copy_from_slice(&encoded[..bytes]);
            }
        }
    }

    fn draw_text(&mut self, x: u32, y: u32, scale: u32, text: &str, color: Rgb) {
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

    /// Redraws the whole status screen.
    pub fn render_status(&mut self, lines: &[String]) {
        let (width, height) = (self.format.width, self.format.height);
        self.fill_rect(0, 0, width, height, BACKGROUND);

        self.draw_text(32, 72, 6, "AMAZING A32", ACCENT);
        self.draw_text(32, 136, 4, "RUST SYSTEM", FOREGROUND);

        let bar = width / 3;
        self.fill_rect(0, 220, bar, 90, RED);
        self.fill_rect(bar, 220, bar, 90, GREEN);
        self.fill_rect(bar * 2, 220, width - bar * 2, 90, BLUE);
        self.draw_text(bar / 2 - 12, 250, 4, "R", BACKGROUND);
        self.draw_text(bar + bar / 2 - 12, 250, 4, "G", BACKGROUND);
        self.draw_text(bar * 2 + bar / 2 - 12, 250, 4, "B", FOREGROUND);

        for (index, line) in lines.iter().enumerate() {
            self.draw_text(32, 360 + index as u32 * 48, 3, line, FOREGROUND);
        }
    }
}

/// Formats the live status lines from raw kernel values.
pub fn status_lines(uptime: &str, capacity: Option<&str>, status: Option<&str>) -> Vec<String> {
    let seconds = uptime
        .split_whitespace()
        .next()
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0) as u64;
    vec![
        format!(
            "UP {:02}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        ),
        format!(
            "BAT {}% {}",
            capacity.map_or("?", str::trim),
            status.map_or("UNKNOWN", str::trim)
        ),
    ]
}

/// Horizontal space per character: five glyph columns plus one blank column.
const GLYPH_ADVANCE: u32 = 6;

/// Returns five columns of a 5x7 glyph, with the top row in bit 0. Lowercase
/// letters use the uppercase shapes, and unknown characters render as blank.
fn glyph(character: char) -> [u8; 5] {
    match character.to_ascii_uppercase() {
        '0' => [0x3E, 0x51, 0x49, 0x45, 0x3E],
        '1' => [0x00, 0x42, 0x7F, 0x40, 0x00],
        '2' => [0x42, 0x61, 0x51, 0x49, 0x46],
        '3' => [0x21, 0x41, 0x45, 0x4B, 0x31],
        '4' => [0x18, 0x14, 0x12, 0x7F, 0x10],
        '5' => [0x27, 0x45, 0x45, 0x45, 0x39],
        '6' => [0x3C, 0x4A, 0x49, 0x49, 0x30],
        '7' => [0x01, 0x71, 0x09, 0x05, 0x03],
        '8' => [0x36, 0x49, 0x49, 0x49, 0x36],
        '9' => [0x06, 0x49, 0x49, 0x29, 0x1E],
        'A' => [0x7E, 0x11, 0x11, 0x11, 0x7E],
        'B' => [0x7F, 0x49, 0x49, 0x49, 0x36],
        'C' => [0x3E, 0x41, 0x41, 0x41, 0x22],
        'D' => [0x7F, 0x41, 0x41, 0x22, 0x1C],
        'E' => [0x7F, 0x49, 0x49, 0x49, 0x41],
        'F' => [0x7F, 0x09, 0x09, 0x09, 0x01],
        'G' => [0x3E, 0x41, 0x49, 0x49, 0x7A],
        'H' => [0x7F, 0x08, 0x08, 0x08, 0x7F],
        'I' => [0x00, 0x41, 0x7F, 0x41, 0x00],
        'J' => [0x20, 0x40, 0x41, 0x3F, 0x01],
        'K' => [0x7F, 0x08, 0x14, 0x22, 0x41],
        'L' => [0x7F, 0x40, 0x40, 0x40, 0x40],
        'M' => [0x7F, 0x02, 0x0C, 0x02, 0x7F],
        'N' => [0x7F, 0x04, 0x08, 0x10, 0x7F],
        'O' => [0x3E, 0x41, 0x41, 0x41, 0x3E],
        'P' => [0x7F, 0x09, 0x09, 0x09, 0x06],
        'Q' => [0x3E, 0x41, 0x51, 0x21, 0x5E],
        'R' => [0x7F, 0x09, 0x19, 0x29, 0x46],
        'S' => [0x46, 0x49, 0x49, 0x49, 0x31],
        'T' => [0x01, 0x01, 0x7F, 0x01, 0x01],
        'U' => [0x3F, 0x40, 0x40, 0x40, 0x3F],
        'V' => [0x1F, 0x20, 0x40, 0x20, 0x1F],
        'W' => [0x3F, 0x40, 0x38, 0x40, 0x3F],
        'X' => [0x63, 0x14, 0x08, 0x14, 0x63],
        'Y' => [0x07, 0x08, 0x70, 0x08, 0x07],
        'Z' => [0x61, 0x51, 0x49, 0x45, 0x43],
        '.' => [0x00, 0x60, 0x60, 0x00, 0x00],
        ':' => [0x00, 0x36, 0x36, 0x00, 0x00],
        '-' => [0x08, 0x08, 0x08, 0x08, 0x08],
        '%' => [0x23, 0x13, 0x08, 0x64, 0x62],
        '?' => [0x02, 0x01, 0x51, 0x09, 0x06],
        _ => [0; 5],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba8888() -> Format {
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
    fn status_screen_shows_all_three_color_bars() {
        let mut canvas = Canvas::new(rgba8888());
        canvas.render_status(&status_lines("5.0 1.0", Some("50"), Some("Charging")));
        assert_eq!(pixel(&canvas, 10, 230), [255, 0, 0, 255]);
        assert_eq!(pixel(&canvas, 170, 230), [0, 255, 0, 255]);
        assert_eq!(pixel(&canvas, 470, 230), [0, 0, 255, 255]);
        assert_eq!(pixel(&canvas, 0, 0), [12, 18, 28, 255]);
    }

    #[test]
    fn long_text_is_clipped_without_panicking() {
        let mut canvas = Canvas::new(rgba8888());
        canvas.draw_text(470, 850, 8, "WIDE TEXT PAST THE EDGE", FOREGROUND);
    }

    #[test]
    fn glyphs_cover_status_text() {
        assert_ne!(glyph('a'), [0; 5]);
        assert_eq!(glyph('a'), glyph('A'));
        assert_eq!(glyph(' '), [0; 5]);
    }

    #[test]
    fn formats_status_lines() {
        assert_eq!(
            status_lines("3725.40 99.0\n", Some("62\n"), Some("Charging\n")),
            vec!["UP 01:02:05", "BAT 62% Charging"]
        );
        assert_eq!(
            status_lines("garbage", None, None),
            vec!["UP 00:00:00", "BAT ?% UNKNOWN"]
        );
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
