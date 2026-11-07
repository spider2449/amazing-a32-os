//! Turns raw camera frames into JPEG photos, and computes the next exposure.
//!
//! The ISP writes Bayer RAW10 frames packed least significant bit first, with
//! red at the top left (R Gr / Gb B). Each 2x2 quad becomes one RGB pixel, so a
//! photo has half the sensor's width and height, with less noise than a full
//! demosaic and no interpolation artifacts. White balance assumes the scene is
//! gray on average, levels are stretched so bright parts reach near white, and
//! a 2.2 gamma follows. Everything here is plain computation, tested on the host.

#[cfg(test)]
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
    // Whole groups of 5 bytes hold 4 values.
    let groups = (width / 4).min(packed.len() / 5);
    for chunk in packed[..groups * 5].chunks_exact(5) {
        let [b0, b1, b2, b3, b4] = [chunk[0], chunk[1], chunk[2], chunk[3], chunk[4]].map(u16::from);
        out.extend_from_slice(&[
            b0 | (b1 & 0x03) << 8,
            b1 >> 2 | (b2 & 0x0F) << 6,
            b2 >> 4 | (b3 & 0x3F) << 4,
            b3 >> 6 | b4 << 2,
        ]);
    }
    // The rest bit by bit.
    let mut bits: u32 = 0;
    let mut count = 0;
    let mut bytes = packed[groups * 5..].iter();
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
    /// Clockwise270, then flipped left to right: the front camera, whose image
    /// came out mirrored without the flip.
    Clockwise270Mirrored,
}

/// Develops a raw frame into an RGB image of half its width and height.
pub fn develop(raw: &[u8], stride: usize, width: usize, height: usize, rotation: Rotation) -> Rgb {
    develop_scaled(raw, stride, width, height, rotation, 1)
}

/// The same, using only every `step`-th quad across and down, so the image is
/// `step` times smaller again and quicker to make; for live video. Bands of
/// rows are worked on by several cores.
pub fn develop_scaled(raw: &[u8], stride: usize, width: usize, height: usize, rotation: Rotation, step: usize) -> Rgb {
    let step = step.max(1);
    let (qw, qh) = (width / (2 * step), height / (2 * step));
    let mut quads = vec![[0u16; 3]; qw * qh];
    if qw == 0 || qh == 0 {
        return Rgb { width: 0, height: 0, data: Vec::new() };
    }

    // Quads, with each band's channel sums for the white balance.
    let line_bytes = width * 10 / 8;
    let band = qh.div_ceil(workers(qh));
    let band_sums: Vec<[u64; 3]> = std::thread::scope(|scope| {
        let bands: Vec<_> = quads
            .chunks_mut(band * qw)
            .enumerate()
            .map(|(index, chunk)| {
                scope.spawn(move || {
                    let (mut top, mut bottom) = (Vec::with_capacity(width), Vec::with_capacity(width));
                    let mut sums = [0u64; 3];
                    for (offset, out) in chunk.chunks_mut(qw).enumerate() {
                        let row = 2 * (index * band + offset) * step * stride;
                        let (Some(a), Some(b)) =
                            (raw.get(row..row + line_bytes), raw.get(row + stride..row + stride + line_bytes))
                        else {
                            break;
                        };
                        unpack_line(a, width, &mut top);
                        unpack_line(b, width, &mut bottom);
                        for qx in 0..qw.min(top.len() / (2 * step)).min(bottom.len() / (2 * step)) {
                            let x = 2 * qx * step;
                            let quad = [top[x], (top[x + 1] + bottom[x]) / 2, bottom[x + 1]];
                            for c in 0..3 {
                                sums[c] += u64::from(quad[c]);
                            }
                            out[qx] = quad;
                        }
                    }
                    sums
                })
            })
            .collect();
        bands.into_iter().map(|band| band.join().expect("develop worker panicked")).collect()
    });

    // Gray world: scale red and blue so their means match green's.
    let mut sums = [0u64; 3];
    for band in band_sums {
        for c in 0..3 {
            sums[c] += band[c];
        }
    }
    let mean = |c: usize| (sums[c] as f32 / quads.len() as f32).max(1.0);
    let gains = [
        (mean(1) / mean(0)).clamp(0.5, 4.0),
        1.0,
        (mean(1) / mean(2)).clamp(0.5, 4.0),
    ];

    // White point: the 99th percentile of the balanced brightest channel. Values
    // are 10-bit, so per-channel tables replace the arithmetic for each quad.
    let balanced: [Vec<u16>; 3] =
        std::array::from_fn(|c| (0..1024).map(|value| (value as f32 * gains[c]) as u16).collect());
    let mut histogram = vec![0u32; 4096];
    for quad in &quads {
        let peak = (0..3).map(|c| balanced[c][usize::from(quad[c] & 0x3FF)]).max().unwrap_or(0);
        histogram[usize::from(peak).min(4095)] += 1;
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
    let tones: [Vec<u8>; 3] = std::array::from_fn(|c| (0..1024).map(|value| tone(value, c)).collect());

    // Output rows in bands, each pixel taken from where the rotation brings it.
    let (out_w, out_h) = match rotation {
        Rotation::None | Rotation::Half => (qw, qh),
        _ => (qh, qw),
    };
    let mut data = vec![0u8; out_w * out_h * 3];
    let band = out_h.div_ceil(workers(out_h));
    let (quads, tones) = (&quads, &tones);
    std::thread::scope(|scope| {
        for (index, chunk) in data.chunks_mut(band * out_w * 3).enumerate() {
            scope.spawn(move || {
                for (offset, row) in chunk.chunks_mut(out_w * 3).enumerate() {
                    let oy = index * band + offset;
                    for (ox, pixel) in row.chunks_exact_mut(3).enumerate() {
                        let (x, y) = match rotation {
                            Rotation::None => (ox, oy),
                            Rotation::Clockwise90 => (oy, qh - 1 - ox),
                            Rotation::Half => (qw - 1 - ox, qh - 1 - oy),
                            Rotation::Clockwise270 => (qw - 1 - oy, ox),
                            Rotation::Clockwise270Mirrored => (qw - 1 - oy, qh - 1 - ox),
                        };
                        let quad = quads[y * qw + x];
                        for c in 0..3 {
                            pixel[c] = tones[c][usize::from(quad[c] & 0x3FF)];
                        }
                    }
                }
            });
        }
    });
    Rgb { width: out_w, height: out_h, data }
}

/// How many threads to split `rows` rows of work between: one per core, at
/// most four (the phone has four).
fn workers(rows: usize) -> usize {
    std::thread::available_parallelism().map_or(1, |count| count.get()).clamp(1, 4).min(rows.max(1))
}

/// Shrinks an image to fit inside `max_width` x `max_height`, keeping its shape,
/// by averaging the source pixels behind each output pixel. Never enlarges.
pub fn fit(image: &Rgb, max_width: usize, max_height: usize) -> Rgb {
    let scale = (max_width as f32 / image.width as f32).min(max_height as f32 / image.height as f32).min(1.0);
    let width = ((image.width as f32 * scale) as usize).max(1);
    let height = ((image.height as f32 * scale) as usize).max(1);
    // The source span behind each output position, from `start` up to `end`.
    let span = |index: usize, out: usize, source: usize| {
        let start = index * source / out;
        (start, ((index + 1) * source / out).max(start + 1))
    };
    let columns: Vec<(usize, usize)> = (0..width).map(|x| span(x, width, image.width)).collect();
    let line = image.width * 3;
    let mut sums = vec![0u32; width * 3];
    let mut data = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        let (top, bottom) = span(y, height, image.height);
        sums.fill(0);
        // Whole source rows at a time, so the columns are worked out once.
        for row in image.data[top * line..bottom * line].chunks_exact(line) {
            for (sum, &(left, right)) in sums.chunks_exact_mut(3).zip(&columns) {
                for pixel in row[left * 3..right * 3].chunks_exact(3) {
                    sum[0] += u32::from(pixel[0]);
                    sum[1] += u32::from(pixel[1]);
                    sum[2] += u32::from(pixel[2]);
                }
            }
        }
        for (sum, &(left, right)) in sums.chunks_exact(3).zip(&columns) {
            let count = ((bottom - top) * (right - left)) as u32;
            data.extend(sum.iter().map(|value| (value / count) as u8));
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
    /// What `fast_dct` output is multiplied by to give quantized values.
    divisors: [f32; 64],
    dc: [(u16, u8); 256],
    ac: [(u16, u8); 256],
}

impl Table {
    fn new(base: &[u8; 64], quality: u32, dc_bits: &[u8; 16], ac_bits: &[u8; 16], ac_values: &[u8]) -> Self {
        let quant = scaled_quant(base, quality);
        let divisors = std::array::from_fn(|k| 1.0 / (f32::from(quant[k]) * AAN_SCALE[k / 8] * AAN_SCALE[k % 8] * 8.0));
        Table { quant, divisors, dc: huffman_codes(dc_bits, &DC_VALUES), ac: huffman_codes(ac_bits, ac_values) }
    }
}

/// Scale factors the AAN DCT leaves in its outputs, by frequency.
const AAN_SCALE: [f32; 8] =
    [1.0, 1.387_039_8, 1.306_563, 1.175_875_6, 1.0, 0.785_694_96, 0.541_196_1, 0.275_899_38];

/// One 8-point pass of the Arai-Agui-Nakajima forward DCT (as in the IJG's
/// jfdctflt.c), on the values at `at`, `at + step`, ... `at + 7 * step`.
fn aan_pass(data: &mut [f32; 64], at: usize, step: usize) {
    let d = |i: usize| data[at + i * step];
    let (tmp0, tmp7) = (d(0) + d(7), d(0) - d(7));
    let (tmp1, tmp6) = (d(1) + d(6), d(1) - d(6));
    let (tmp2, tmp5) = (d(2) + d(5), d(2) - d(5));
    let (tmp3, tmp4) = (d(3) + d(4), d(3) - d(4));

    let (tmp10, tmp13) = (tmp0 + tmp3, tmp0 - tmp3);
    let (tmp11, tmp12) = (tmp1 + tmp2, tmp1 - tmp2);
    let z1 = (tmp12 + tmp13) * 0.707_106_77;
    let even = [tmp10 + tmp11, tmp13 + z1, tmp10 - tmp11, tmp13 - z1];

    let (tmp10, tmp11, tmp12) = (tmp4 + tmp5, tmp5 + tmp6, tmp6 + tmp7);
    let z5 = (tmp10 - tmp12) * 0.382_683_43;
    let z2 = 0.541_196_1 * tmp10 + z5;
    let z4 = 1.306_563 * tmp12 + z5;
    let z3 = tmp11 * 0.707_106_77;
    let (z11, z13) = (tmp7 + z3, tmp7 - z3);

    let out = [even[0], z11 + z4, even[1], z13 - z2, even[2], z13 + z2, even[3], z11 - z4];
    for (i, value) in out.into_iter().enumerate() {
        data[at + i * step] = value;
    }
}

/// Forward DCT of a level-shifted 8x8 block in place, in natural order, with
/// each output still scaled by AAN_SCALE[row] * AAN_SCALE[column] * 8.
fn fast_dct(data: &mut [f32; 64]) {
    for row in 0..8 {
        aan_pass(data, row * 8, 1);
    }
    for column in 0..8 {
        aan_pass(data, column, 8);
    }
}

/// The plain DCT, to check `fast_dct` against.
#[cfg(test)]
struct Dct {
    cos: [[f32; 8]; 8],
}

#[cfg(test)]
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

fn encode_block(writer: &mut BitWriter, block: &mut [f32; 64], table: &Table, previous_dc: &mut i32) {
    fast_dct(block);
    let mut quantized = [0i32; 64];
    for (k, &natural) in ZIGZAG.iter().enumerate() {
        quantized[k] = (block[natural] * table.divisors[natural]).round() as i32;
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
    let luma = Table::new(&LUMA_QUANT, quality, &DC_LUMA_BITS, &AC_LUMA_BITS, &AC_LUMA_VALUES);
    let chroma = Table::new(&CHROMA_QUANT, quality, &DC_CHROMA_BITS, &AC_CHROMA_BITS, &AC_CHROMA_VALUES);

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
    // A restart marker after each row of macroblocks makes the rows independent,
    // so several cores can encode them.
    let mcu_rows = height.div_ceil(16);
    segment(&mut out, 0xDD, &(width.div_ceil(16) as u16).to_be_bytes());
    segment(&mut out, 0xDA, &[3, 1, 0x00, 2, 0x11, 3, 0x11, 0, 63, 0]);
    let threads = workers(mcu_rows);
    let per_thread = mcu_rows.div_ceil(threads);
    let (luma, chroma) = (&luma, &chroma);
    let rows: Vec<Vec<u8>> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..mcu_rows)
            .step_by(per_thread)
            .map(|first| {
                scope.spawn(move || {
                    (first..(first + per_thread).min(mcu_rows))
                        .map(|row| encode_mcu_row(image, row, luma, chroma))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers.into_iter().flat_map(|worker| worker.join().expect("JPEG worker panicked")).collect()
    });
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            out.extend_from_slice(&[0xFF, 0xD0 + ((index - 1) % 8) as u8]);
        }
        out.extend_from_slice(row);
    }
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

/// Copies an 8x8 block out of a plane.
fn load_block(block: &mut [f32; 64], plane: &[f32], plane_width: usize, left: usize, top: usize) {
    for y in 0..8 {
        let start = (top + y) * plane_width + left;
        block[y * 8..y * 8 + 8].copy_from_slice(&plane[start..start + 8]);
    }
}

/// Encodes one row of 16x16 macroblocks as an entropy-coded segment: level
/// shifted Y, Cb and Cr with edges repeated, and Cb and Cr averaged over each
/// 2x2 pixels.
fn encode_mcu_row(image: &Rgb, mcu_row: usize, luma: &Table, chroma: &Table) -> Vec<u8> {
    let (width, height) = (image.width, image.height);
    let padded_w = width.div_ceil(16) * 16;
    let chroma_w = padded_w / 2;
    let mut y_plane = vec![0.0f32; padded_w * 16];
    let mut cb_plane = vec![0.0f32; chroma_w * 8];
    let mut cr_plane = vec![0.0f32; chroma_w * 8];
    for row in 0..16 {
        let source = (mcu_row * 16 + row).min(height - 1) * width;
        for x in 0..padded_w {
            let at = (source + x.min(width - 1)) * 3;
            let (r, g, b) = (f32::from(image.data[at]), f32::from(image.data[at + 1]), f32::from(image.data[at + 2]));
            y_plane[row * padded_w + x] = 0.299 * r + 0.587 * g + 0.114 * b - 128.0;
            let c = (row / 2) * chroma_w + x / 2;
            cb_plane[c] += (-0.168_736 * r - 0.331_264 * g + 0.5 * b) / 4.0;
            cr_plane[c] += (0.5 * r - 0.418_688 * g - 0.081_312 * b) / 4.0;
        }
    }
    let mut writer = BitWriter { out: Vec::with_capacity(padded_w * 8), buffer: 0, count: 0 };
    let mut dc = [0i32; 3];
    let mut block = [0.0f32; 64];
    for mx in (0..padded_w).step_by(16) {
        for (by, bx) in [(0, 0), (0, 8), (8, 0), (8, 8)] {
            load_block(&mut block, &y_plane, padded_w, mx + bx, by);
            encode_block(&mut writer, &mut block, luma, &mut dc[0]);
        }
        load_block(&mut block, &cb_plane, chroma_w, mx / 2, 0);
        encode_block(&mut writer, &mut block, chroma, &mut dc[1]);
        load_block(&mut block, &cr_plane, chroma_w, mx / 2, 0);
        encode_block(&mut writer, &mut block, chroma, &mut dc[2]);
    }
    writer.flush();
    writer.out
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
        // A restart interval of one macroblock row: 37 pixels are 3 macroblocks.
        let dri = jpeg.windows(2).position(|w| w == [0xFF, 0xDD]).unwrap();
        assert_eq!(&jpeg[dri + 2..dri + 6], &[0, 4, 0, 3]);
        // Every 0xFF in the entropy data is stuffed or a restart marker, and the
        // markers count up: 21 lines are 2 rows, so one marker.
        let sos = jpeg.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
        let data = &jpeg[sos + 14..jpeg.len() - 2];
        let mut restarts = Vec::new();
        for (i, &byte) in data.iter().enumerate() {
            if byte == 0xFF {
                match data.get(i + 1) {
                    Some(0) => {}
                    Some(&marker) => restarts.push(marker),
                    None => panic!("0xFF at the end"),
                }
            }
        }
        assert_eq!(restarts, [0xD0]);
        // Taller images cycle through all eight markers.
        let tall = Rgb { width: 16, height: 16 * 10, data: vec![128; 16 * 160 * 3] };
        let jpeg = encode_jpeg(&tall, 85);
        let markers: Vec<u8> =
            jpeg.windows(2).filter(|w| w[0] == 0xFF && (0xD0..=0xD7).contains(&w[1])).map(|w| w[1]).collect();
        assert_eq!(markers, [0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD0]);
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
    fn mirrored_rotation_flips_the_turned_image_left_to_right() {
        // 4x4 raw of four quads, all different.
        let rows = [[100, 100, 300, 300], [100, 100, 300, 300], [600, 600, 900, 900], [600, 600, 900, 900]];
        let raw: Vec<u8> = rows.iter().flat_map(|row| pack_lsb(row)).collect();
        let turned = develop(&raw, 5, 4, 4, Rotation::Clockwise270);
        let mirrored = develop(&raw, 5, 4, 4, Rotation::Clockwise270Mirrored);
        assert_eq!((mirrored.width, mirrored.height), (turned.width, turned.height));
        let flip = |image: &Rgb| -> Vec<u8> {
            image.data.chunks(image.width * 3).flat_map(|row| row.chunks(3).rev().flatten().copied().collect::<Vec<_>>()).collect()
        };
        assert_eq!(mirrored.data, flip(&turned));
        assert_ne!(mirrored.data, turned.data);
    }

    /// Pseudo-random numbers, the same on every run.
    fn noise(count: usize, seed: u32, max: u32) -> Vec<u32> {
        let mut state = seed;
        (0..count)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (state >> 8) % (max + 1)
            })
            .collect()
    }

    #[test]
    fn fast_dct_matches_the_plain_one() {
        let dct = Dct::new();
        for seed in 1..20 {
            let values = noise(64, seed, 255);
            let block: [f32; 64] = std::array::from_fn(|i| values[i] as f32 - 128.0);
            let plain = dct.forward(&block);
            let mut fast = block;
            fast_dct(&mut fast);
            for k in 0..64 {
                let scaled = fast[k] / (AAN_SCALE[k / 8] * AAN_SCALE[k % 8] * 8.0);
                assert!((scaled - plain[k]).abs() < 0.01, "seed {seed} k {k}: {scaled} vs {}", plain[k]);
            }
        }
    }

    #[test]
    fn unpacks_whole_groups_and_the_rest() {
        for width in [1, 3, 4, 7, 8, 13, 64] {
            let values: Vec<u16> = noise(width, width as u32, 1023).into_iter().map(|v| v as u16).collect();
            // Lines are padded to whole bytes; pack_lsb only writes whole bytes.
            let mut padded = values.clone();
            padded.resize(width.div_ceil(4) * 4, 0);
            let mut out = Vec::new();
            unpack_line(&pack_lsb(&padded), width, &mut out);
            assert_eq!(out, values, "width {width}");
        }
        // A short line gives what it holds.
        let mut out = Vec::new();
        unpack_line(&pack_lsb(&[1, 2, 3, 4]), 8, &mut out);
        assert_eq!(out, [1, 2, 3, 4]);
    }

    #[test]
    fn scaled_develop_skips_quads() {
        // 8x4 raw: quads (0,0) dark, (1,0) bright, (2,0) dark, (3,0) bright; the
        // second quad row is skipped at step 2.
        let row = pack_lsb(&[100, 100, 900, 900, 100, 100, 900, 900]);
        let skipped = pack_lsb(&[1023; 8]);
        let raw = [row.clone(), row, skipped.clone(), skipped].concat();
        let image = develop_scaled(&raw, 10, 8, 4, Rotation::None, 2);
        assert_eq!((image.width, image.height), (2, 1));
        // Both kept quads are the dark ones (x = 0 and x = 4), so they match.
        assert_eq!(image.data[0..3], image.data[3..6]);
        let full = develop_scaled(&raw, 10, 8, 4, Rotation::None, 1);
        assert_eq!((full.width, full.height), (4, 2));
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

    /// The first `fit`, one output pixel at a time, which the faster one must match.
    fn fit_by_pixel(image: &Rgb, max_width: usize, max_height: usize) -> Rgb {
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

    #[test]
    fn fit_matches_the_pixel_by_pixel_version() {
        // Live frames (rear, front), a photo, and odd sizes.
        for (width, height, max_width, max_height) in
            [(486, 648, 480, 640), (600, 800, 480, 640), (972, 1296, 480, 640), (7, 5, 3, 3), (5, 9, 100, 4)]
        {
            let data = (0..width * height * 3).map(|i| (i * 7919 % 251) as u8).collect();
            let image = Rgb { width, height, data };
            let (fast, slow) = (fit(&image, max_width, max_height), fit_by_pixel(&image, max_width, max_height));
            assert_eq!((fast.width, fast.height), (slow.width, slow.height));
            assert!(fast.data == slow.data, "{width}x{height} into {max_width}x{max_height} differs");
        }
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
