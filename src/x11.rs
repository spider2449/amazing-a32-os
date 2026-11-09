//! Just enough of the X11 protocol for the init to show a window on the desktop:
//! the connection setup, a few requests, and the events a camera window needs.
//! This only builds and reads bytes, so it runs on the host in unit tests. All
//! numbers are little-endian, which the setup request asks for.

use crate::display::{Channel, Format};

// Request opcodes.
const CREATE_WINDOW: u8 = 1;
const DESTROY_WINDOW: u8 = 4;
const MAP_WINDOW: u8 = 8;
const CONFIGURE_WINDOW: u8 = 12;
const INTERN_ATOM: u8 = 16;
const CHANGE_PROPERTY: u8 = 18;
const CREATE_GC: u8 = 55;
const PUT_IMAGE: u8 = 72;

// Predefined atoms.
pub const ATOM: u32 = 4;
pub const STRING: u32 = 31;
pub const WM_NAME: u32 = 39;
pub const WM_NORMAL_HINTS: u32 = 40;
pub const WM_SIZE_HINTS: u32 = 41;
pub const WM_CLASS: u32 = 67;

// Event masks.
const BUTTON_PRESS_MASK: u32 = 1 << 2;
const EXPOSURE_MASK: u32 = 1 << 15;
const STRUCTURE_NOTIFY_MASK: u32 = 1 << 17;

/// What the server told about itself and its first screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    pub id_base: u32,
    pub id_mask: u32,
    /// The longest request, in 4-byte units.
    pub max_request_units: u16,
    /// Whether images are least significant byte first.
    pub image_lsb_first: bool,
    pub root: u32,
    pub black_pixel: u32,
    pub width: u16,
    pub height: u16,
    pub depth: u8,
    pub bits_per_pixel: u8,
    /// Each image row is padded to this many bits.
    pub scanline_pad: u8,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
}

impl Setup {
    /// The `n`th resource id this client may use.
    pub fn id(&self, n: u32) -> u32 {
        self.id_base | (n & self.id_mask)
    }

    /// A pixel layout for images of this size that `put_image` can send as they
    /// are, or why there is none.
    pub fn image_format(&self, width: u32, height: u32) -> Result<Format, String> {
        if !self.image_lsb_first {
            return Err("the X server wants images most significant byte first".to_string());
        }
        if !matches!(self.bits_per_pixel, 16 | 24 | 32) {
            return Err(format!("unsupported X pixel size: {} bits", self.bits_per_pixel));
        }
        let channel = |mask: u32| Channel { offset: mask.trailing_zeros(), length: mask.count_ones() };
        let pad = u32::from(self.scanline_pad.max(8));
        let row_bits = width * u32::from(self.bits_per_pixel);
        Ok(Format {
            width,
            height,
            stride: row_bits.div_ceil(pad) * pad / 8,
            bits_per_pixel: u32::from(self.bits_per_pixel),
            red: channel(self.red_mask),
            green: channel(self.green_mask),
            blue: channel(self.blue_mask),
            alpha: Channel::default(),
        })
    }
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn pad4(length: usize) -> usize {
    length.div_ceil(4) * 4
}

/// The first bytes a client sends: little-endian, protocol 11.0, no authorization.
/// The desktop's X server is started without any.
pub fn setup_request() -> [u8; 12] {
    [b'l', 0, 11, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

/// How many more bytes follow the first 8 of the server's setup reply.
pub fn setup_reply_length(header: &[u8; 8]) -> usize {
    usize::from(u16_at(header, 6)) * 4
}

/// Reads the server's setup reply: its first 8 bytes and the rest.
pub fn parse_setup(header: &[u8; 8], body: &[u8]) -> Result<Setup, String> {
    if header[0] != 1 {
        // A refusal gives the reason's length in byte 1, and the reason first.
        let reason_length = usize::from(header[1]).min(body.len());
        let reason = String::from_utf8_lossy(&body[..reason_length]);
        return Err(format!("the X server refused the connection: {reason}"));
    }
    let short = || "the X server's setup reply is too short".to_string();
    // Offsets below are from the start of the whole reply, 8 bytes before `body`.
    let at = |offset: usize| offset - 8;
    if body.len() < at(40) {
        return Err(short());
    }
    let vendor_length = usize::from(u16_at(body, at(24)));
    let screens = body[at(28)];
    let format_count = usize::from(body[at(29)]);
    let formats = at(40) + pad4(vendor_length);
    let screen = formats + format_count * 8;
    if screens == 0 {
        return Err("the X server has no screen".to_string());
    }
    if body.len() < screen + 40 {
        return Err(short());
    }
    let root_visual = u32_at(body, screen + 32);
    let depth = body[screen + 38];
    let (bits_per_pixel, scanline_pad) = (0..format_count)
        .map(|index| &body[formats + index * 8..formats + index * 8 + 8])
        .find(|format| format[0] == depth)
        .map(|format| (format[1], format[2]))
        .ok_or_else(|| format!("the X server has no image format for depth {depth}"))?;

    // The root visual's color masks, from the screen's list of depths.
    let mut at_depth = screen + 40;
    let mut masks = None;
    for _ in 0..body[screen + 39] {
        if body.len() < at_depth + 8 {
            return Err(short());
        }
        let visuals = usize::from(u16_at(body, at_depth + 2));
        let first = at_depth + 8;
        if body.len() < first + visuals * 24 {
            return Err(short());
        }
        for visual in (0..visuals).map(|index| &body[first + index * 24..first + index * 24 + 24]) {
            if u32_at(visual, 0) == root_visual {
                masks = Some((u32_at(visual, 8), u32_at(visual, 12), u32_at(visual, 16)));
            }
        }
        at_depth = first + visuals * 24;
    }
    let (red_mask, green_mask, blue_mask) = masks.ok_or("the X server's root visual is not listed")?;
    Ok(Setup {
        id_base: u32_at(body, at(12)),
        id_mask: u32_at(body, at(16)),
        max_request_units: u16_at(body, at(26)),
        image_lsb_first: body[at(30)] == 0,
        root: u32_at(body, screen),
        black_pixel: u32_at(body, screen + 12),
        width: u16_at(body, screen + 20),
        height: u16_at(body, screen + 22),
        depth,
        bits_per_pixel,
        scanline_pad,
        red_mask,
        green_mask,
        blue_mask,
    })
}

/// A request with its header: the opcode, one byte of data, and the length in
/// 4-byte units, which counts the header. `body` is padded to a whole unit.
fn request(opcode: u8, data: u8, body: &[u8]) -> Vec<u8> {
    let length = 4 + pad4(body.len());
    let mut out = Vec::with_capacity(length);
    out.push(opcode);
    out.push(data);
    out.extend_from_slice(&((length / 4) as u16).to_le_bytes());
    out.extend_from_slice(body);
    out.resize(length, 0);
    out
}

fn words(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|value| value.to_le_bytes()).collect()
}

/// Creates a window of the screen's depth on the root, with a black background,
/// that is told about clicks, exposure, and changes to its size.
pub fn create_window(setup: &Setup, window: u32, x: i16, y: i16, width: u16, height: u16) -> Vec<u8> {
    const BACK_PIXEL: u32 = 1 << 1;
    const EVENT_MASK: u32 = 1 << 11;
    const INPUT_OUTPUT: u16 = 1;
    let mut body = words(&[window, setup.root]);
    for value in [x as u16, y as u16, width, height, 0, INPUT_OUTPUT] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    // The visual is copied from the parent, then the values in mask order.
    body.extend(words(&[
        0,
        BACK_PIXEL | EVENT_MASK,
        setup.black_pixel,
        BUTTON_PRESS_MASK | EXPOSURE_MASK | STRUCTURE_NOTIFY_MASK,
    ]));
    request(CREATE_WINDOW, setup.depth, &body)
}

pub fn map_window(window: u32) -> Vec<u8> {
    request(MAP_WINDOW, 0, &words(&[window]))
}

pub fn destroy_window(window: u32) -> Vec<u8> {
    request(DESTROY_WINDOW, 0, &words(&[window]))
}

/// Puts the window above the others.
pub fn raise_window(window: u32) -> Vec<u8> {
    const STACK_MODE: u16 = 1 << 6;
    const ABOVE: u32 = 0;
    let mut body = words(&[window]);
    body.extend_from_slice(&STACK_MODE.to_le_bytes());
    body.extend_from_slice(&[0, 0]);
    body.extend(words(&[ABOVE]));
    request(CONFIGURE_WINDOW, 0, &body)
}

/// Asks for the atom of `name`, making it if needed. The reply's atom is read by
/// `reply_atom`.
pub fn intern_atom(name: &str) -> Vec<u8> {
    let mut body = (name.len() as u16).to_le_bytes().to_vec();
    body.extend_from_slice(&[0, 0]);
    body.extend_from_slice(name.as_bytes());
    request(INTERN_ATOM, 0, &body)
}

/// The atom in an `intern_atom` reply, which is 32 bytes.
pub fn reply_atom(reply: &[u8; 32]) -> Option<u32> {
    (reply[0] == 1).then(|| u32_at(reply, 8))
}

/// Replaces a property of a window with 8-bit data.
pub fn set_property_bytes(window: u32, property: u32, kind: u32, data: &[u8]) -> Vec<u8> {
    set_property(window, property, kind, 8, data.len() as u32, data)
}

/// Replaces a property of a window with 32-bit data.
pub fn set_property_words(window: u32, property: u32, kind: u32, data: &[u32]) -> Vec<u8> {
    set_property(window, property, kind, 32, data.len() as u32, &words(data))
}

fn set_property(window: u32, property: u32, kind: u32, format: u8, count: u32, data: &[u8]) -> Vec<u8> {
    const REPLACE: u8 = 0;
    let mut body = words(&[window, property, kind]);
    body.extend_from_slice(&[format, 0, 0, 0]);
    body.extend(words(&[count]));
    body.extend_from_slice(data);
    request(CHANGE_PROPERTY, REPLACE, &body)
}

/// The window manager's size hints, asking for this place and size.
pub fn size_hints(x: i32, y: i32, width: u32, height: u32) -> [u32; 18] {
    const P_POSITION: u32 = 1 << 2;
    const P_SIZE: u32 = 1 << 3;
    let mut hints = [0u32; 18];
    hints[0] = P_POSITION | P_SIZE;
    hints[1] = x as u32;
    hints[2] = y as u32;
    hints[3] = width;
    hints[4] = height;
    hints
}

/// A graphics context with the default values, for `put_image`.
pub fn create_gc(gc: u32, drawable: u32) -> Vec<u8> {
    request(CREATE_GC, 0, &words(&[gc, drawable, 0]))
}

/// Draws an image given in the server's own pixel format, `rows` rows of
/// `row_bytes` each, padded as the setup asks, at (x, y).
pub fn put_image(setup: &Setup, drawable: u32, gc: u32, width: u16, x: i16, y: i16, row_bytes: usize, pixels: &[u8]) -> Vec<u8> {
    const Z_PIXMAP: u8 = 2;
    let rows = (pixels.len() / row_bytes.max(1)) as u16;
    let mut body = Vec::with_capacity(20 + pixels.len());
    body.extend(words(&[drawable, gc]));
    for value in [width, rows, x as u16, y as u16] {
        body.extend_from_slice(&value.to_le_bytes());
    }
    body.extend_from_slice(&[0, setup.depth, 0, 0]);
    body.extend_from_slice(pixels);
    request(PUT_IMAGE, Z_PIXMAP, &body)
}

/// How many rows of `row_bytes` fit in one `put_image` request.
pub fn rows_per_put(setup: &Setup, row_bytes: usize) -> usize {
    let limit = usize::from(setup.max_request_units) * 4;
    (limit.saturating_sub(24) / row_bytes.max(1)).max(1)
}

/// What the server sent, in the 32 bytes every event, error and reply starts with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Error { code: u8, major: u8, value: u32 },
    /// A reply, with this many more bytes after the first 32.
    Reply { extra: usize },
    ButtonPress { button: u8, x: i16, y: i16 },
    /// Part of the window needs drawing; `count` more such events follow.
    Expose { count: u16 },
    /// The window's size is now this.
    Configure { window: u32, width: u16, height: u16 },
    ClientMessage { kind: u32, data: u32 },
    Destroyed { window: u32 },
    Other(u8),
}

pub fn parse_event(bytes: &[u8; 32]) -> Event {
    // The top bit says the event was sent by another client.
    match bytes[0] & 0x7F {
        0 => Event::Error { code: bytes[1], major: bytes[10], value: u32_at(bytes, 4) },
        1 => Event::Reply { extra: u32_at(bytes, 4) as usize * 4 },
        4 => Event::ButtonPress { button: bytes[1], x: u16_at(bytes, 24) as i16, y: u16_at(bytes, 26) as i16 },
        12 => Event::Expose { count: u16_at(bytes, 16) },
        17 => Event::Destroyed { window: u32_at(bytes, 8) },
        22 => Event::Configure { window: u32_at(bytes, 8), width: u16_at(bytes, 20), height: u16_at(bytes, 22) },
        33 => Event::ClientMessage { kind: u32_at(bytes, 8), data: u32_at(bytes, 12) },
        code => Event::Other(code),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A setup reply like Xorg's on the fbdev driver: depth 24 in 32-bit pixels,
    /// one screen of 480x854 with one TrueColor visual.
    fn reply() -> ([u8; 8], Vec<u8>) {
        let vendor = b"The X.Org Foundation";
        let mut body = Vec::new();
        body.extend(words(&[12_101_011, 0x0020_0000, 0x001F_FFFF, 256]));
        body.extend_from_slice(&(vendor.len() as u16).to_le_bytes());
        body.extend_from_slice(&65535u16.to_le_bytes());
        body.extend_from_slice(&[1, 2, 0, 0, 32, 32, 8, 255, 0, 0, 0, 0]);
        body.extend_from_slice(vendor);
        body.resize(body.len() + pad4(vendor.len()) - vendor.len(), 0);
        body.extend_from_slice(&[1, 1, 32, 0, 0, 0, 0, 0]);
        body.extend_from_slice(&[24, 32, 32, 0, 0, 0, 0, 0]);
        // The screen.
        body.extend(words(&[0x1E3, 0x20, 0xFF_FFFF, 0, 0]));
        for value in [480u16, 854, 64, 113, 1, 1] {
            body.extend_from_slice(&value.to_le_bytes());
        }
        body.extend(words(&[0x21]));
        body.extend_from_slice(&[0, 0, 24, 1]);
        // Its one depth with one visual.
        body.extend_from_slice(&[24, 0, 1, 0, 0, 0, 0, 0]);
        body.extend(words(&[0x21]));
        body.extend_from_slice(&[4, 8, 0, 1]);
        body.extend(words(&[0xFF_0000, 0xFF00, 0xFF, 0]));
        let mut header = [1, 0, 11, 0, 0, 0, 0, 0];
        header[6..8].copy_from_slice(&((body.len() / 4) as u16).to_le_bytes());
        (header, body)
    }

    #[test]
    fn reads_the_setup_reply() {
        let (header, body) = reply();
        assert_eq!(setup_reply_length(&header), body.len());
        let setup = parse_setup(&header, &body).unwrap();
        assert_eq!(setup.root, 0x1E3);
        assert_eq!((setup.width, setup.height, setup.depth, setup.bits_per_pixel), (480, 854, 24, 32));
        assert_eq!((setup.red_mask, setup.green_mask, setup.blue_mask), (0xFF_0000, 0xFF00, 0xFF));
        assert_eq!(setup.id(3), 0x0020_0003);
        assert!(setup.image_lsb_first);
        let format = setup.image_format(101, 10).unwrap();
        assert_eq!(format.stride, 404);
        assert_eq!(format.red, Channel { offset: 16, length: 8 });
        assert_eq!(format.blue, Channel { offset: 0, length: 8 });
        // Too short anywhere fails instead of panicking.
        for cut in 0..body.len() {
            assert!(parse_setup(&header, &body[..cut]).is_err(), "cut at {cut}");
        }
    }

    #[test]
    fn a_refusal_gives_its_reason() {
        let header = [0, 6, 11, 0, 0, 0, 2, 0];
        let error = parse_setup(&header, b"no way\0\0").unwrap_err();
        assert!(error.ends_with("no way"), "{error}");
    }

    #[test]
    fn requests_are_padded_and_count_their_length() {
        let (header, body) = reply();
        let setup = parse_setup(&header, &body).unwrap();
        let atom = intern_atom("WM_PROTOCOLS");
        assert_eq!(atom.len(), 20);
        assert_eq!(&atom[..4], &[INTERN_ATOM, 0, 5, 0]);
        let window = create_window(&setup, 7, 0, 0, 480, 790);
        assert_eq!(window.len(), 40);
        assert_eq!(&window[..4], &[CREATE_WINDOW, 24, 10, 0]);
        let name = set_property_bytes(7, WM_NAME, STRING, b"Camera");
        assert_eq!(name.len(), 32);
        assert_eq!(u16_at(&name, 2), 8);
        let image = put_image(&setup, 7, 8, 2, 0, 5, 8, &[0; 24]);
        assert_eq!(image.len(), 24 + 24);
        assert_eq!(u16_at(&image, 14), 3);
        assert_eq!(rows_per_put(&setup, 480 * 4), 136);
        assert_eq!(raise_window(7).len(), 16);
    }

    #[test]
    fn reads_events() {
        let mut press = [0u8; 32];
        press[0] = 4;
        press[1] = 1;
        press[24..26].copy_from_slice(&120u16.to_le_bytes());
        press[26..28].copy_from_slice(&40u16.to_le_bytes());
        assert_eq!(parse_event(&press), Event::ButtonPress { button: 1, x: 120, y: 40 });
        let mut configure = [0u8; 32];
        configure[0] = 22;
        configure[8] = 7;
        configure[20..22].copy_from_slice(&480u16.to_le_bytes());
        configure[22..24].copy_from_slice(&760u16.to_le_bytes());
        assert_eq!(parse_event(&configure), Event::Configure { window: 7, width: 480, height: 760 });
        let mut sent = [0u8; 32];
        sent[0] = 0x80 | 33;
        sent[8] = 9;
        sent[12] = 10;
        assert_eq!(parse_event(&sent), Event::ClientMessage { kind: 9, data: 10 });
        let mut reply = [0u8; 32];
        reply[0] = 1;
        reply[4] = 2;
        reply[8] = 0x55;
        assert_eq!(parse_event(&reply), Event::Reply { extra: 8 });
        assert_eq!(reply_atom(&reply), Some(0x55));
    }
}
