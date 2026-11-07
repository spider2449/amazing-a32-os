//! Turns raw camera frames into JPEG photos, and computes the next exposure.
//!
//! The ISP writes Bayer RAW10 frames packed least significant bit first, with
//! red at the top left (R Gr / Gb B). Each 2x2 quad becomes one RGB pixel, so a
//! photo has half the sensor's width and height, with less noise than a full
//! demosaic and no interpolation artifacts. White balance assumes the scene is
//! gray on average, levels are stretched so bright parts reach near white, and
//! a 2.2 gamma follows. Everything here is plain computation, tested on the host.

use std::f32::consts::PI;

/// Exposure in sensor terms: shutter in lines and analog gain with 64 = 1x.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Exposure {
    pub shutter: u32,
    pub gain: u32,
}

/// What a sensor allows for `Exposure`.
#[derive(Clone, Copy, Debug)]
pub struct ExposureLimits {
    pub max_shutter: u32,
    pub max_gain: u32,
}

pub const UNIT_GAIN: u32 = 64;
/// Raw mean level (10-bit) that auto exposure aims for.
pub const TARGET_LEVEL: f32 = 180.0;

/// An 8-bit RGB image, rows top to bottom.
pub struct Rgb {
    pub width: usize,
    pub height: usize,
    pub data: Vec<u8>,
}

/// Unpacks one line of RAW10 packed least significant bit first.
pub fn unpack_line(packed: &[u8], width: usize, out: &mut Vec<u16>) {
    out.clear();
    let mut bits: u32 = 0;
    let mut count = 0;
    let mut bytes = packed.iter();
    while out.len() < width {
        while count < 10 {
            let Some(&byte) = bytes.next() else { return };
            bits |= u32::from(byte) << count;
            count += 8;
        }
        out.push((bits & 0x3FF) as u16);
        bits >>= 10;
        count -= 10;
    }
}

/// The mean raw level of a frame, from every `step`-th pixel of every `step`-th
/// line, for auto exposure.
pub fn mean_level(raw: &[u8], stride: usize, width: usize, height: usize, step: usize) -> f32 {
    let mut line = Vec::with_capacity(width);
    let (mut sum, mut count) = (0u64, 0u64);
    for y in (0..height).step_by(step) {
        let Some(packed) = raw.get(y * stride..y * stride + width * 10 / 8) else { break };
        unpack_line(packed, width, &mut line);
        for value in line.iter().step_by(step) {
            sum += u64::from(*value);
            count += 1;
        }
    }
    if count == 0 {
        0.0
    } else {
        sum as f32 / count as f32
    }
}

/// The exposure that should bring `mean` to `TARGET_LEVEL`: longer shutter first,
/// then more gain, and the reverse when darker is needed. A change is at most 8x
/// per step, because a saturated frame says little about the right exposure.
pub fn next_exposure(current: Exposure, mean: f32, limits: ExposureLimits) -> Exposure {
    let ratio = (TARGET_LEVEL / mean.max(0.5)).clamp(0.125, 8.0);
    let total = (current.shutter as f32 * current.gain as f32 * ratio).max(UNIT_GAIN as f32);
    let shutter = (total / UNIT_GAIN as f32).clamp(1.0, limits.max_shutter as f32);
    let gain = (total / shutter).clamp(UNIT_GAIN as f32, limits.max_gain as f32);
    Exposure { shutter: shutter.round() as u32, gain: gain.round() as u32 }
}

/// Whether `mean` is close enough to the target to stop adjusting.
pub fn exposure_settled(mean: f32, current: Exposure, limits: ExposureLimits) -> bool {
    let close = (mean - TARGET_LEVEL).abs() < TARGET_LEVEL * 0.2;
    let at_max = current.shutter >= limits.max_shutter && current.gain >= limits.max_gain && mean < TARGET_LEVEL;
    let at_min = current.shutter <= 1 && current.gain <= UNIT_GAIN && mean > TARGET_LEVEL;
    close || at_max || at_min
}

/// Rotation applied to the sensor image, clockwise.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum Rotation {
    None,
    Clockwise90,
    Half,
    Clockwise270,
}

/// Develops a raw frame into an RGB image of half its width and height.
pub fn develop(raw: &[u8], stride: usize, width: usize, height: usize, rotation: Rotation) -> Rgb {
    let (qw, qh) = (width / 2, height / 2);
    let mut quads = vec![[0u16; 3]; qw * qh];
    let (mut top, mut bottom) = (Vec::with_capacity(width), Vec::with_capacity(width));
    let line_bytes = width * 10 / 8;
    for qy in 0..qh {
        let row = 2 * qy * stride;
        let (Some(a), Some(b)) = (raw.get(row..row + line_bytes), raw.get(row + stride..row + stride + line_bytes))
        else {
            break;
        };
        unpack_line(a, width, &mut top);
        unpack_line(b, width, &mut bottom);
        for qx in 0..qw.min(top.len() / 2).min(bottom.len() / 2) {
            let (r, gr) = (top[2 * qx], top[2 * qx + 1]);
            let (gb, b) = (bottom[2 * qx], bottom[2 * qx + 1]);
            quads[qy * qw + qx] = [r, (gr + gb) / 2, b];
        }
    }

    // Gray world: scale red and blue so their means match green's.
    let mut sums = [0u64; 3];
    for quad in &quads {
        for c in 0..3 {
            sums[c] += u64::from(quad[c]);
        }
    }
    let mean = |c: usize| (sums[c] as f32 / quads.len().max(1) as f32).max(1.0);
    let gains = [
        (mean(1) / mean(0)).clamp(0.5, 4.0),
        1.0,
        (mean(1) / mean(2)).clamp(0.5, 4.0),
    ];

    // White point: the 99th percentile of the balanced brightest channel.
    let mut histogram = vec![0u32; 4096];
    for quad in &quads {
        let peak = (0..3).map(|c| quad[c] as f32 * gains[c]).fold(0.0, f32::max);
        histogram[(peak as usize).min(4095)] += 1;
    }
    let wanted = quads.len() as u32 / 100;
    let mut seen = 0;
    let mut white = 4095;
    for level in (0..4096).rev() {
        seen += histogram[level];
        if seen > wanted {
            white = level;
            break;
        }
    }
    let white = (white as f32).max(64.0);

    // Gamma 2.2 lookup over the scaled range.
    let lut: Vec<u8> = (0..=1024)
        .map(|i| (255.0 * (i as f32 / 1024.0).powf(1.0 / 2.2)).round() as u8)
        .collect();
    let tone = |value: u16, c: usize| -> u8 {
        let scaled = (value as f32 * gains[c] / white * 1024.0) as usize;
        lut[scaled.min(1024)]
    };

    let (out_w, out_h) = match rotation {
        Rotation::None | Rotation::Half => (qw, qh),
        _ => (qh, qw),
    };
    let mut data = vec![0u8; out_w * out_h * 3];
    for y in 0..qh {
        for x in 0..qw {
            let (ox, oy) = match rotation {
                Rotation::None => (x, y),
                Rotation::Clockwise90 => (qh - 1 - y, x),
                Rotation::Half => (qw - 1 - x, qh - 1 - y),
                Rotation::Clockwise270 => (y, qw - 1 - x),
            };
            let quad = quads[y * qw + x];
            let at = (oy * out_w + ox) * 3;
            for c in 0..3 {
                data[at + c] = tone(quad[c], c);
            }
        }
    }
    Rgb { width: out_w, height: out_h, data }
}

/// Shrinks an image to fit inside `max_width` x `max_height`, keeping its shape,
/// by averaging the source pixels behind each output pixel. Never enlarges.
pub fn fit(image: &Rgb, max_width: usize, max_height: usize) -> Rgb {
    let scale = (max_width as f32 / image.width as f32).min(max_height as f32 / image.height as f32).min(1.0);
    let width = ((image.width as f32 * scale) as usize).max(1);
    let height = ((image.height as f32 * scale) as usize).max(1);
    let mut data = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        let (top, bottom) = (y * image.height / height, ((y + 1) * image.height / height).max(y * image.height / height + 1));
        for x in 0..width {
            let (left, right) = (x * image.width / width, ((x + 1) * image.width / width).max(x * image.width / width + 1));
            let mut sums = [0u32; 3];
            for sy in top..bottom {
                for sx in left..right {
                    let at = (sy * image.width + sx) * 3;
                    for c in 0..3 {
                        sums[c] += u32::from(image.data[at + c]);
                    }
                }
            }
            let count = ((bottom - top) * (right - left)) as u32;
            data.extend(sums.iter().map(|sum| (sum / count) as u8));
        }
    }
    Rgb { width, height, data }
}

// ---------------------------------------------------------------------------
// Baseline JPEG encoder: 4:2:0 chroma, the example tables of ITU-T T.81 Annex K.

const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21,
    28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61,
    54, 47, 55, 62, 63,
];

const LUMA_QUANT: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56, 14, 17, 22, 29,
    51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49, 64, 78, 87, 103, 121,
    120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

const CHROMA_QUANT: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99, 47, 66, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
];

const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7D];
const AC_LUMA_VALUES: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14,
    0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09,
    0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A,
    0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65,
    0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88,
    0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9,
    0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA,
    0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA,
    0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA,
];

const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHROMA_VALUES: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71, 0x13, 0x22, 0x32,
    0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0, 0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16,
    0x24, 0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39,
    0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64,
    0x65, 0x66, 0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82, 0x83, 0x84, 0x85, 0x86,
    0x87, 0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
    0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8,
    0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9,
    0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA,
];

/// Canonical Huffman codes for a table: (code, length) indexed by symbol.
fn huffman_codes(bits: &[u8; 16], values: &[u8]) -> [(u16, u8); 256] {
    let mut codes = [(0u16, 0u8); 256];
    let mut code: u16 = 0;
    let mut k = 0;
    for (i, &count) in bits.iter().enumerate() {
        for _ in 0..count {
            codes[values[k] as usize] = (code, i as u8 + 1);
            code += 1;
            k += 1;
        }
        code <<= 1;
    }
    codes
}

/// Quantization table for `quality` (1-100), the IJG way, in natural order.
fn scaled_quant(base: &[u8; 64], quality: u32) -> [u16; 64] {
    let quality = quality.clamp(1, 100);
    let scale = if quality < 50 { 5000 / quality } else { 200 - 2 * quality };
    let mut out = [0u16; 64];
    for i in 0..64 {
        out[i] = ((u32::from(base[i]) * scale + 50) / 100).clamp(1, 255) as u16;
    }
    out
}

struct BitWriter {
    out: Vec<u8>,
    buffer: u32,
    count: u32,
}

impl BitWriter {
    fn put(&mut self, code: u16, length: u8) {
        self.buffer = (self.buffer << length) | u32::from(code);
        self.count += u32::from(length);
        while self.count >= 8 {
            let byte = (self.buffer >> (self.count - 8)) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0); // byte stuffing
            }
            self.count -= 8;
        }
        self.buffer &= (1 << self.count) - 1;
    }

    fn flush(&mut self) {
        if self.count > 0 {
            let pad = 8 - self.count as u8;
            self.put((1 << pad) - 1, pad); // pad with one bits
        }
    }
}

/// The JPEG magnitude category of a coefficient and its additional bits.
fn category(value: i32) -> (u8, u16) {
    let magnitude = value.unsigned_abs();
    let size = 32 - magnitude.leading_zeros();
    let bits = if value < 0 { value - 1 } else { value };
    (size as u8, (bits as u32 & ((1u32 << size) - 1)) as u16)
}

struct Table {
    quant: [u16; 64],
    dc: [(u16, u8); 256],
    ac: [(u16, u8); 256],
}

struct Dct {
    cos: [[f32; 8]; 8],
}

impl Dct {
    fn new() -> Self {
        let mut cos = [[0.0; 8]; 8];
        for u in 0..8 {
            let scale = if u == 0 { (0.125f32).sqrt() } else { 0.5 };
            for x in 0..8 {
                cos[u][x] = scale * ((2 * x + 1) as f32 * u as f32 * PI / 16.0).cos();
            }
        }
        Dct { cos }
    }

    /// Forward DCT of a level-shifted 8x8 block, in natural order.
    fn forward(&self, block: &[f32; 64]) -> [f32; 64] {
        let mut rows = [0.0f32; 64];
        for y in 0..8 {
            for u in 0..8 {
                rows[y * 8 + u] = (0..8).map(|x| self.cos[u][x] * block[y * 8 + x]).sum();
            }
        }
        let mut out = [0.0f32; 64];
        for u in 0..8 {
            for v in 0..8 {
                out[v * 8 + u] = (0..8).map(|y| self.cos[v][y] * rows[y * 8 + u]).sum();
            }
        }
        out
    }
}

fn encode_block(writer: &mut BitWriter, dct: &Dct, block: &[f32; 64], table: &Table, previous_dc: &mut i32) {
    let coefficients = dct.forward(block);
    let mut quantized = [0i32; 64];
    for (k, &natural) in ZIGZAG.iter().enumerate() {
        quantized[k] = (coefficients[natural] / f32::from(table.quant[natural])).round() as i32;
    }
    let diff = quantized[0] - *previous_dc;
    *previous_dc = quantized[0];
    let (size, bits) = category(diff);
    let (code, length) = table.dc[size as usize];
    writer.put(code, length);
    if size > 0 {
        writer.put(bits, size);
    }
    let mut run = 0;
    for &value in &quantized[1..] {
        if value == 0 {
            run += 1;
            continue;
        }
        while run > 15 {
            let (code, length) = table.ac[0xF0];
            writer.put(code, length);
            run -= 16;
        }
        let (size, bits) = category(value);
        let (code, length) = table.ac[(run << 4 | size as usize) & 0xFF];
        writer.put(code, length);
        writer.put(bits, size);
        run = 0;
    }
    if run > 0 {
        let (code, length) = table.ac[0x00];
        writer.put(code, length);
    }
}

fn segment(out: &mut Vec<u8>, marker: u8, body: &[u8]) {
    out.extend_from_slice(&[0xFF, marker]);
    out.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(body);
}

fn huffman_segment(out: &mut Vec<u8>, class_id: u8, bits: &[u8; 16], values: &[u8]) {
    let mut body = vec![class_id];
    body.extend_from_slice(bits);
    body.extend_from_slice(values);
    segment(out, 0xC4, &body);
}

/// Encodes an RGB image as a baseline JPEG with 4:2:0 chroma subsampling.
pub fn encode_jpeg(image: &Rgb, quality: u32) -> Vec<u8> {
    let (width, height) = (image.width, image.height);
    let luma = Table {
        quant: scaled_quant(&LUMA_QUANT, quality),
        dc: huffman_codes(&DC_LUMA_BITS, &DC_VALUES),
        ac: huffman_codes(&AC_LUMA_BITS, &AC_LUMA_VALUES),
    };
    let chroma = Table {
        quant: scaled_quant(&CHROMA_QUANT, quality),
        dc: huffman_codes(&DC_CHROMA_BITS, &DC_VALUES),
        ac: huffman_codes(&AC_CHROMA_BITS, &AC_CHROMA_VALUES),
    };

    let mut out = vec![0xFF, 0xD8];
    segment(&mut out, 0xE0, b"JFIF\0\x01\x01\x00\x00\x01\x00\x01\x00\x00");
    for (id, table) in [(0u8, &luma), (1, &chroma)] {
        let mut body = vec![id];
        body.extend(ZIGZAG.iter().map(|&natural| table.quant[natural] as u8));
        segment(&mut out, 0xDB, &body);
    }
    let mut sof = vec![8];
    sof.extend_from_slice(&(height as u16).to_be_bytes());
    sof.extend_from_slice(&(width as u16).to_be_bytes());
    sof.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    segment(&mut out, 0xC0, &sof);
    huffman_segment(&mut out, 0x00, &DC_LUMA_BITS, &DC_VALUES);
    huffman_segment(&mut out, 0x10, &AC_LUMA_BITS, &AC_LUMA_VALUES);
    huffman_segment(&mut out, 0x01, &DC_CHROMA_BITS, &DC_VALUES);
    huffman_segment(&mut out, 0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALUES);
    segment(&mut out, 0xDA, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);

    // Pixel at (x, y) with edges repeated, as Y, Cb, Cr.
    let ycc = |x: usize, y: usize| -> [f32; 3] {
        let at = (y.min(height - 1) * width + x.min(width - 1)) * 3;
        let (r, g, b) = (image.data[at] as f32, image.data[at + 1] as f32, image.data[at + 2] as f32);
        [
            0.299 * r + 0.587 * g + 0.114 * b,
            -0.168_736 * r - 0.331_264 * g + 0.5 * b + 128.0,
            0.5 * r - 0.418_688 * g - 0.081_312 * b + 128.0,
        ]
    };

    let dct = Dct::new();
    let mut writer = BitWriter { out, buffer: 0, count: 0 };
    let mut dc = [0i32; 3];
    let mut block = [0.0f32; 64];
    for my in (0..height).step_by(16) {
        for mx in (0..width).step_by(16) {
            for (by, bx) in [(0, 0), (0, 8), (8, 0), (8, 8)] {
                for y in 0..8 {
                    for x in 0..8 {
                        block[y * 8 + x] = ycc(mx + bx + x, my + by + y)[0] - 128.0;
                    }
                }
                encode_block(&mut writer, &dct, &block, &luma, &mut dc[0]);
            }
            for component in 1..3 {
                for y in 0..8 {
                    for x in 0..8 {
                        let mut sum = 0.0;
                        for (dy, dx) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                            sum += ycc(mx + 2 * x + dx, my + 2 * y + dy)[component];
                        }
                        block[y * 8 + x] = sum / 4.0 - 128.0;
                    }
                }
                encode_block(&mut writer, &dct, &block, &chroma, &mut dc[component]);
            }
        }
    }
    writer.flush();
    let mut out = writer.out;
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack_lsb(values: &[u16]) -> Vec<u8> {
        let mut out = Vec::new();
        let (mut bits, mut count) = (0u32, 0);
        for &value in values {
            bits |= u32::from(value) << count;
            count += 10;
            while count >= 8 {
                out.push(bits as u8);
                bits >>= 8;
                count -= 8;
            }
        }
        out
    }

    #[test]
    fn unpacks_raw10_least_significant_bit_first() {
        let values = [0, 1, 0x3FF, 0x155, 0x2AA, 7, 512, 1000];
        let mut out = Vec::new();
        unpack_line(&pack_lsb(&values), values.len(), &mut out);
        assert_eq!(out, values);
    }

    #[test]
    fn huffman_tables_are_complete() {
        for (bits, values) in [
            (&DC_LUMA_BITS, &DC_VALUES[..]),
            (&DC_CHROMA_BITS, &DC_VALUES[..]),
            (&AC_LUMA_BITS, &AC_LUMA_VALUES[..]),
            (&AC_CHROMA_BITS, &AC_CHROMA_VALUES[..]),
        ] {
            let total: usize = bits.iter().map(|&b| b as usize).sum();
            assert_eq!(total, values.len());
            // No code is all ones, and lengths fit: the Kraft sum stays below 1.
            let kraft: f64 = bits.iter().enumerate().map(|(i, &b)| b as f64 / 2f64.powi(i as i32 + 1)).sum();
            assert!(kraft < 1.0);
            let mut sorted = values.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), values.len(), "duplicate symbols");
        }
    }

    #[test]
    fn categories_follow_the_standard() {
        assert_eq!(category(0), (0, 0));
        assert_eq!(category(1), (1, 1));
        assert_eq!(category(-1), (1, 0));
        assert_eq!(category(5), (3, 5));
        assert_eq!(category(-5), (3, 2));
        assert_eq!(category(1023), (10, 1023));
    }

    #[test]
    fn quality_scales_quantization() {
        assert_eq!(scaled_quant(&LUMA_QUANT, 50)[0], 16);
        assert_eq!(scaled_quant(&LUMA_QUANT, 100), [1; 64]);
        assert!(scaled_quant(&LUMA_QUANT, 10)[0] > 16);
    }

    #[test]
    fn writes_a_well_formed_jpeg() {
        let image = Rgb { width: 37, height: 21, data: (0..37 * 21 * 3).map(|i| (i * 7 % 256) as u8).collect() };
        let jpeg = encode_jpeg(&image, 85);
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
        assert_eq!(&jpeg[jpeg.len() - 2..], &[0xFF, 0xD9]);
        // Every 0xFF in the entropy data is stuffed or is a marker we wrote.
        let sos = jpeg.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
        let data = &jpeg[sos + 14..jpeg.len() - 2];
        for (i, &byte) in data.iter().enumerate() {
            if byte == 0xFF {
                assert_eq!(data.get(i + 1), Some(&0));
            }
        }
    }

    /// With A32_TEST_JPEG=PATH set, writes a color test chart there, so that an
    /// independent decoder can check the encoder.
    #[test]
    fn writes_a_test_chart_on_request() {
        let Ok(path) = std::env::var("A32_TEST_JPEG") else { return };
        let (width, height) = (320, 200);
        let mut data = Vec::with_capacity(width * height * 3);
        for y in 0..height {
            for x in 0..width {
                let pixel = match (x / 80, y / 100) {
                    (0, 0) => [255, 0, 0],
                    (1, 0) => [0, 255, 0],
                    (2, 0) => [0, 0, 255],
                    (3, 0) => [255, 255, 255],
                    _ => [(x * 255 / width) as u8; 3],
                };
                data.extend_from_slice(&pixel);
            }
        }
        std::fs::write(path, encode_jpeg(&Rgb { width, height, data }, 90)).unwrap();
    }

    #[test]
    fn develops_quads_with_rotation() {
        // 4x2 raw: two quads, left dark, right bright.
        let top = pack_lsb(&[100, 100, 900, 900]);
        let bottom = pack_lsb(&[100, 100, 900, 900]);
        let raw = [top, bottom].concat();
        let image = develop(&raw, 5, 4, 2, Rotation::None);
        assert_eq!((image.width, image.height), (2, 1));
        assert!(image.data[0] < image.data[3]);
        let turned = develop(&raw, 5, 4, 2, Rotation::Clockwise90);
        assert_eq!((turned.width, turned.height), (1, 2));
        assert!(turned.data[0] < turned.data[3]); // left quad is now on top
    }

    #[test]
    fn exposure_moves_toward_the_target() {
        let limits = ExposureLimits { max_shutter: 3000, max_gain: 512 };
        let dark = next_exposure(Exposure { shutter: 1000, gain: 64 }, 45.0, limits);
        assert_eq!(dark, Exposure { shutter: 3000, gain: 85 }); // 4x: shutter first, then gain
        let bright = next_exposure(Exposure { shutter: 3000, gain: 256 }, 1020.0, limits);
        assert_eq!(bright.gain, 64);
        assert!(bright.shutter < 3000);
        let capped = next_exposure(Exposure { shutter: 3000, gain: 512 }, 1.0, limits);
        assert_eq!(capped, Exposure { shutter: 3000, gain: 512 });
        assert!(exposure_settled(170.0, Exposure { shutter: 10, gain: 64 }, limits));
        assert!(exposure_settled(20.0, capped, limits));
        assert!(!exposure_settled(20.0, Exposure { shutter: 10, gain: 64 }, limits));
    }

    #[test]
    fn fit_keeps_the_shape_and_averages() {
        // 4x2: left half black, right half white.
        let data = (0..8).flat_map(|i| if i % 4 < 2 { [0u8; 3] } else { [200u8; 3] }).collect();
        let small = fit(&Rgb { width: 4, height: 2, data }, 2, 10);
        assert_eq!((small.width, small.height), (2, 1));
        assert_eq!(small.data, vec![0, 0, 0, 200, 200, 200]);
        let same = fit(&small, 100, 100);
        assert_eq!((same.width, same.height), (2, 1));
        let portrait = fit(&Rgb { width: 972, height: 1296, data: vec![9; 972 * 1296 * 3] }, 480, 640);
        assert_eq!((portrait.width, portrait.height), (480, 640));
    }

    #[test]
    fn mean_level_samples_the_frame() {
        let line = pack_lsb(&[200; 8]);
        let raw = [line.clone(), line].concat();
        assert_eq!(mean_level(&raw, 10, 8, 2, 1), 200.0);
    }
}
