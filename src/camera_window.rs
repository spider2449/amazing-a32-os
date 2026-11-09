//! The desktop's Camera program: a window that the init draws itself as an X11
//! client, since the init owns the cameras. It shows live video, switches between
//! the rear and front cameras, and saves photos to root's Pictures folder in
//! Alpine, where the Files program finds them.
//!
//! JWM's menu runs `SCRIPT`, which writes to a FIFO that a thread here reads. A
//! second request raises the window that is open.

use crate::alpine;
use crate::camera;
use crate::desktop;
use crate::display::{Canvas, Format, Rect, Rgb, ACCENT, BACKGROUND, FOREGROUND};
use crate::photo;
use crate::pid1::{log, take_photo};
use crate::x11::{self, Event, Setup};
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::raw::{c_char, c_int};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, Once};
use std::thread;
use std::time::Duration;

extern "C" {
    fn mkfifo(path: *const c_char, mode: u32) -> c_int;
}

/// The script JWM's menu runs, and the FIFO it writes to. /run is shared with the
/// chroot, so these paths are the same on both sides.
pub const SCRIPT: &str = crate::jwmrc::CAMERA_SCRIPT;
const REQUESTS: &str = "/run/a32-camera-request";
/// The window's class, which JWM's settings give a group (see `jwmrc`).
pub const CLASS: &str = crate::jwmrc::CAMERA_CLASS;
const TITLE: &str = "Camera";

const PICTURES: &str = "/data/alpine/root/Pictures";
const PICTURES_SHOWN: &str = "~/Pictures";
/// Live video pauses after this long, so a forgotten window does not keep the
/// camera running; a tap on the picture goes on.
const LIVE_LIMIT: Duration = Duration::from_secs(10 * 60);

/// The size asked for before the window manager maximizes it: the screen less
/// JWM's tray and a title bar.
const TRAY_AND_TITLE: u16 = 34 + 30;

const BAR_HEIGHT: u32 = 64;
const STATUS_HEIGHT: u32 = 28;
const BUTTON: Rgb = Rgb(44, 58, 80);
const BLACK: Rgb = Rgb(0, 0, 0);
const TEXT_LARGE: u32 = 3;
const TEXT_SMALL: u32 = 2;
/// A character's width at scale 1, as `Canvas` draws text.
const GLYPH_ADVANCE: u32 = 6;

/// The script that asks for the window.
pub fn script() -> String {
    format!("#!/bin/sh\n# Opens the camera window, or raises it.\necho open > {REQUESTS}\n")
}

static LISTENING: Once = Once::new();

/// Starts the thread that opens the window on request, once.
pub fn listen() {
    LISTENING.call_once(|| {
        let _ = fs::remove_file(REQUESTS);
        let path = CString::new(REQUESTS).expect("no NUL in the path");
        // SAFETY: the path is a valid NUL-terminated string.
        if unsafe { mkfifo(path.as_ptr(), 0o622) } != 0 {
            log(&format!("camera window: {REQUESTS}: {}", io::Error::last_os_error()));
            return;
        }
        let spawned = thread::Builder::new().name("camera requests".to_string()).spawn(|| loop {
            // Opening waits for a writer, and reading ends when the last one closes.
            let mut text = String::new();
            match File::open(REQUESTS).and_then(|mut fifo| fifo.read_to_string(&mut text)) {
                Ok(_) if !text.trim().is_empty() => open(),
                Ok(_) => {}
                Err(err) => {
                    log(&format!("camera window: {REQUESTS}: {err}"));
                    thread::sleep(Duration::from_secs(1));
                }
            }
        });
        if let Err(err) = spawned {
            log(&format!("camera window: {err}"));
        }
    });
}

enum Message {
    Raise,
    Event(Event),
    Disconnected(String),
    /// The live view with this number has a new frame in the slot, or ended.
    Frame(u32),
    LiveEnded(u32, Option<String>),
}

/// How to reach the open window, if there is one.
static WINDOW: Mutex<Option<mpsc::Sender<Message>>> = Mutex::new(None);

/// Opens the window, or raises it if it is open.
fn open() {
    let mut window = WINDOW.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(sender) = window.as_ref() {
        if sender.send(Message::Raise).is_ok() {
            return;
        }
    }
    let (sender, messages) = mpsc::channel();
    let own = sender.clone();
    let spawned = thread::Builder::new().name("camera window".to_string()).spawn(move || {
        log("camera window: opening");
        let ended = run(own, messages);
        *WINDOW.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        match ended {
            Ok(()) => log("camera window: closed"),
            Err(err) => log(&format!("camera window: {err}")),
        }
    });
    match spawned {
        Ok(_) => *window = Some(sender),
        Err(err) => log(&format!("camera window: {err}")),
    }
}

/// Live video in a thread, which leaves only the newest frame in `slot`.
struct Live {
    stop: Arc<AtomicBool>,
    thread: thread::JoinHandle<()>,
}

impl Live {
    fn start(
        name: &'static str,
        number: u32,
        slot: Arc<Mutex<Option<photo::Rgb>>>,
        window: mpsc::Sender<Message>,
    ) -> io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = thread::Builder::new().name("camera window live".to_string()).spawn(move || {
            let Some(camera) = camera::by_name(name) else { return };
            log(&format!("camera window: {name} live view"));
            let result = camera::live(camera, LIVE_LIMIT, |image| {
                if stopping.load(Ordering::Acquire) {
                    return Err(io::Error::from(io::ErrorKind::Interrupted));
                }
                let mut waiting = slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                // Only an empty slot needs telling the window; otherwise it is told already.
                if waiting.replace(image).is_none() && window.send(Message::Frame(number)).is_err() {
                    return Err(io::Error::from(io::ErrorKind::BrokenPipe));
                }
                Ok(())
            });
            let message = match result {
                Ok(summary) => {
                    let seconds = summary.elapsed.as_secs_f32().max(0.001);
                    log(&format!(
                        "camera window: {name} live view ended: {} frames {}x{} in {:.0} s ({:.1} fps)",
                        summary.sent,
                        summary.width,
                        summary.height,
                        seconds,
                        summary.sent as f32 / seconds,
                    ));
                    // Stopped by the window, which says what happens next itself.
                    (!stopping.load(Ordering::Acquire)).then(|| {
                        format!("Paused after {} min. Tap the picture to go on.", LIVE_LIMIT.as_secs() / 60)
                    })
                }
                Err(err) => {
                    log(&format!("camera window: {name} live view failed: {err}"));
                    Some(format!("Camera: {err}. Tap the picture to try again."))
                }
            };
            let _ = window.send(Message::LiveEnded(number, message));
        })?;
        Ok(Self { stop, thread })
    }

    /// Stops the live view and waits until the camera is free.
    fn stop(self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.thread.join();
    }
}

/// The window and what it shows.
struct Window {
    stream: UnixStream,
    setup: Setup,
    id: u32,
    gc: u32,
    format: Format,
    canvas: Canvas,
    camera: &'static str,
    status: String,
    /// The last picture drawn, fitted to the window, for drawing again.
    picture: Option<photo::Rgb>,
}

/// The window's live view, numbered so that frames and endings of one that was
/// stopped are told from those of the next.
struct LiveControl {
    number: u32,
    slot: Arc<Mutex<Option<photo::Rgb>>>,
    window: mpsc::Sender<Message>,
    running: Option<Live>,
}

impl LiveControl {
    /// Starts live video from the window's camera, or says on the window why not.
    fn start(&mut self, window: &mut Window) {
        self.number += 1;
        match Live::start(window.camera, self.number, self.slot.clone(), self.window.clone()) {
            Ok(started) => self.running = Some(started),
            Err(err) => window.status = format!("Camera: {err}"),
        }
    }

    fn stop(&mut self) {
        if let Some(running) = self.running.take() {
            running.stop();
        }
        self.slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
    }

    /// The waiting frame, if it is from the view that runs.
    fn frame(&self, number: u32) -> Option<photo::Rgb> {
        let image = self.slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).take();
        image.filter(|_| number == self.number && self.running.is_some())
    }
}

/// Connects to the desktop's X server and runs the window until it is closed or
/// the desktop ends.
fn run(sender: mpsc::Sender<Message>, messages: mpsc::Receiver<Message>) -> io::Result<()> {
    let (stream, setup) = connect()?;
    let mut window = Window::create(stream, setup)?;
    let (protocols, delete) = (window.atom("WM_PROTOCOLS")?, window.atom("WM_DELETE_WINDOW")?);
    window.send(&x11::set_property_words(window.id, protocols, x11::ATOM, &[delete]))?;
    window.send(&x11::map_window(window.id))?;
    hide_keyboard();

    let reader = window.stream.try_clone()?;
    let events = sender.clone();
    thread::Builder::new().name("camera window events".to_string()).spawn(move || read_events(reader, events))?;

    let mut live = LiveControl { number: 0, slot: Arc::new(Mutex::new(None)), window: sender, running: None };
    live.start(&mut window);
    window.draw_all()?;

    let result = loop {
        let Ok(message) = messages.recv() else { break Ok(()) };
        let drawn = match message {
            Message::Raise => window.send(&x11::map_window(window.id)).and_then(|()| window.send(&x11::raise_window(window.id))),
            Message::Disconnected(why) => break Err(io::Error::other(why)),
            Message::Frame(frame) => match live.frame(frame) {
                Some(image) => window.show(&image),
                None => Ok(()),
            },
            Message::LiveEnded(ended, message) => {
                if ended == live.number {
                    live.stop();
                    if let Some(message) = message {
                        window.status = message;
                    }
                }
                window.draw_status()
            }
            Message::Event(Event::ClientMessage { kind, data }) if kind == protocols && data == delete => break Ok(()),
            Message::Event(Event::Destroyed { window: id }) if id == window.id => break Ok(()),
            Message::Event(Event::Configure { window: id, width, height }) if id == window.id => {
                window.resize(u32::from(width), u32::from(height))
            }
            Message::Event(Event::Expose { count: 0 }) => window.draw_all(),
            Message::Event(Event::ButtonPress { button: 1, x, y }) => {
                let (x, y) = (i32::from(x), i32::from(y));
                let width = window.format.width;
                if switch_button(width).contains(x, y) {
                    live.stop();
                    window.camera = other_camera(window.camera);
                    window.status = format!("{} camera", capitalized(window.camera));
                    window.picture = None;
                    live.start(&mut window);
                    window.draw_all()
                } else if photo_button(width).contains(x, y) {
                    live.stop();
                    let taken = window.take_photo();
                    live.start(&mut window);
                    taken.and_then(|()| window.draw_all())
                } else if live.running.is_none() && picture_area(width, window.format.height).contains(x, y) {
                    window.status = format!("{} camera", capitalized(window.camera));
                    live.start(&mut window);
                    window.draw_status()
                } else {
                    Ok(())
                }
            }
            Message::Event(_) => Ok(()),
        };
        if let Err(err) = drawn {
            break Err(err);
        }
    };
    live.stop();
    let _ = window.send(&x11::destroy_window(window.id));
    // Ends the event thread too.
    let _ = window.stream.shutdown(Shutdown::Both);
    result
}

/// Opens the connection and reads the server's setup.
fn connect() -> io::Result<(UnixStream, Setup)> {
    let mut stream = UnixStream::connect(desktop::SOCKET)
        .map_err(|err| io::Error::new(err.kind(), format!("the desktop's X server: {err}")))?;
    stream.write_all(&x11::setup_request())?;
    let mut header = [0u8; 8];
    stream.read_exact(&mut header)?;
    let mut body = vec![0u8; x11::setup_reply_length(&header)];
    stream.read_exact(&mut body)?;
    let setup = x11::parse_setup(&header, &body).map_err(io::Error::other)?;
    Ok((stream, setup))
}

/// Reads what the server sends and hands the events to the window, until the
/// connection closes.
fn read_events(mut stream: UnixStream, window: mpsc::Sender<Message>) {
    let mut bytes = [0u8; 32];
    loop {
        if let Err(err) = stream.read_exact(&mut bytes) {
            let _ = window.send(Message::Disconnected(format!("the X server is gone: {err}")));
            return;
        }
        let event = x11::parse_event(&bytes);
        match event {
            // Nothing asks for a reply once the window runs, but one must not
            // leave the rest out of step.
            Event::Reply { extra } => {
                if io::copy(&mut (&mut stream).take(extra as u64), &mut io::sink()).is_err() {
                    return;
                }
            }
            Event::Error { code, major, value } => {
                log(&format!("camera window: X error {code} for request {major}, value {value:#x}"));
            }
            _ => {}
        }
        if window.send(Message::Event(event)).is_err() {
            return;
        }
    }
}

/// The keyboard sits over the bottom of the screen and is of no use here; the KB
/// button in the tray shows it again.
fn hide_keyboard() {
    let line = format!("pkill -x {}", crate::jwmrc::KEYBOARD);
    if let Err(err) = alpine::spawn("/bin/sh", &["-c", &line], &[], "/dev/null") {
        log(&format!("camera window: hiding the keyboard: {err}"));
    }
}

fn other_camera(name: &str) -> &'static str {
    if name == "rear" {
        "front"
    } else {
        "rear"
    }
}

fn capitalized(name: &str) -> String {
    let mut characters = name.chars();
    characters.next().map_or_else(String::new, |first| first.to_uppercase().chain(characters).collect())
}

fn switch_button(width: u32) -> Rect {
    Rect { x: 8, y: 8, width: (width / 2).saturating_sub(12), height: BAR_HEIGHT - 16 }
}

fn photo_button(width: u32) -> Rect {
    Rect { x: width / 2 + 4, y: 8, width: (width / 2).saturating_sub(12), height: BAR_HEIGHT - 16 }
}

fn status_area(width: u32) -> Rect {
    Rect { x: 0, y: BAR_HEIGHT, width, height: STATUS_HEIGHT }
}

fn picture_area(width: u32, height: u32) -> Rect {
    let top = BAR_HEIGHT + STATUS_HEIGHT;
    Rect { x: 0, y: top, width, height: height.saturating_sub(top) }
}

impl Window {
    fn create(stream: UnixStream, setup: Setup) -> io::Result<Self> {
        let (id, gc) = (setup.id(1), setup.id(2));
        let width = setup.width;
        let height = setup.height.saturating_sub(TRAY_AND_TITLE).max(1);
        let format = setup.image_format(u32::from(width), u32::from(height)).map_err(io::Error::other)?;
        let mut window = Self {
            stream,
            id,
            gc,
            canvas: Canvas::new(format),
            format,
            setup,
            camera: "rear",
            status: "Rear camera".to_string(),
            picture: None,
        };
        window.send(&x11::create_window(&window.setup, id, 0, 0, width, height))?;
        window.send(&x11::set_property_bytes(id, x11::WM_NAME, x11::STRING, TITLE.as_bytes()))?;
        // The tray's task list shows the class as the name, not the title; JWM's
        // settings name it too.
        let class = format!("{CLASS}\0{CLASS}\0");
        window.send(&x11::set_property_bytes(id, x11::WM_CLASS, x11::STRING, class.as_bytes()))?;
        let hints = x11::size_hints(0, 0, u32::from(width), u32::from(height));
        window.send(&x11::set_property_words(id, x11::WM_NORMAL_HINTS, x11::WM_SIZE_HINTS, &hints))?;
        window.send(&x11::create_gc(gc, id))?;
        Ok(window)
    }

    fn send(&mut self, request: &[u8]) -> io::Result<()> {
        self.stream.write_all(request)
    }

    /// Asks for an atom and waits for it. Only used before the event thread runs.
    fn atom(&mut self, name: &str) -> io::Result<u32> {
        self.send(&x11::intern_atom(name))?;
        let mut bytes = [0u8; 32];
        loop {
            self.stream.read_exact(&mut bytes)?;
            match x11::parse_event(&bytes) {
                Event::Reply { .. } => {
                    return x11::reply_atom(&bytes).ok_or_else(|| io::Error::other(format!("no atom {name}")))
                }
                Event::Error { code, .. } => return Err(io::Error::other(format!("X error {code} for atom {name}"))),
                // An event before the window is mapped, such as a ConfigureNotify, is dropped.
                _ => {}
            }
        }
    }

    fn resize(&mut self, width: u32, height: u32) -> io::Result<()> {
        if (width, height) == (self.format.width, self.format.height) || width == 0 || height == 0 {
            return Ok(());
        }
        self.format = self.setup.image_format(width, height).map_err(io::Error::other)?;
        self.canvas = Canvas::new(self.format);
        // The picture was fitted to the old size; the next frame fits the new one.
        self.picture = None;
        self.draw_all()
    }

    fn draw_all(&mut self) -> io::Result<()> {
        let width = self.format.width;
        self.canvas.fill(Rect { x: 0, y: 0, width, height: BAR_HEIGHT }, BACKGROUND);
        let switch = format!("TO {}", other_camera(self.camera).to_uppercase());
        self.canvas.fill(switch_button(width), BUTTON);
        self.canvas.draw_text_centered(switch_button(width), TEXT_LARGE, &switch, FOREGROUND);
        self.canvas.fill(photo_button(width), ACCENT);
        self.canvas.draw_text_centered(photo_button(width), TEXT_LARGE, "PHOTO", BLACK);
        self.paint_status();
        self.paint_picture();
        self.present(Rect { x: 0, y: 0, width, height: self.format.height })
    }

    fn draw_status(&mut self) -> io::Result<()> {
        self.paint_status();
        self.present(status_area(self.format.width))
    }

    fn paint_status(&mut self) {
        let area = status_area(self.format.width);
        self.canvas.fill(area, BACKGROUND);
        let fits = (area.width / (GLYPH_ADVANCE * TEXT_SMALL)) as usize;
        let text: String = self.status.chars().take(fits).collect();
        self.canvas.draw_text_centered(area, TEXT_SMALL, &text, FOREGROUND);
    }

    fn paint_picture(&mut self) {
        let area = picture_area(self.format.width, self.format.height);
        self.canvas.fill(area, BLACK);
        if let Some(picture) = &self.picture {
            let (width, height) = (picture.width as u32, picture.height as u32);
            let x = area.x + area.width.saturating_sub(width) / 2;
            let y = area.y + area.height.saturating_sub(height) / 2;
            self.canvas.draw_rgb(x, y, width, height, &picture.data);
        }
    }

    /// Shows a frame or photo, made to fit the window.
    fn show(&mut self, image: &photo::Rgb) -> io::Result<()> {
        let area = picture_area(self.format.width, self.format.height);
        if area.width == 0 || area.height == 0 {
            return Ok(());
        }
        let fitted = photo::fit(image, area.width as usize, area.height as usize);
        // Only a change of size leaves parts of the old picture to clear.
        let same_size = self.picture.as_ref().is_some_and(|old| (old.width, old.height) == (fitted.width, fitted.height));
        self.picture = Some(fitted);
        self.paint_picture();
        let picture = self.picture.as_ref().expect("just set");
        let shown = if same_size {
            let (width, height) = (picture.width as u32, picture.height as u32);
            Rect {
                x: area.x + area.width.saturating_sub(width) / 2,
                y: area.y + area.height.saturating_sub(height) / 2,
                width,
                height,
            }
        } else {
            area
        };
        self.present(shown)
    }

    /// Takes a photo with the current camera and saves it, saying how it went.
    fn take_photo(&mut self) -> io::Result<()> {
        self.status = "Taking a photo...".to_string();
        self.draw_status()?;
        match take_photo(self.camera) {
            Ok(photo) => {
                self.status = match save(&photo.jpeg) {
                    Ok(name) => format!("Saved {PICTURES_SHOWN}/{name}"),
                    Err(err) => format!("Not saved: {err}"),
                };
                self.picture = None;
                self.show(&photo.image)
            }
            Err(err) => {
                self.status = format!("Photo failed: {err}");
                Ok(())
            }
        }
    }

    /// Copies part of the canvas to the window, in as many requests as it takes.
    fn present(&mut self, area: Rect) -> io::Result<()> {
        let bytes = (self.format.bits_per_pixel / 8) as usize;
        let stride = self.format.stride as usize;
        let right = (area.x + area.width).min(self.format.width);
        let bottom = (area.y + area.height).min(self.format.height);
        if area.x >= right || area.y >= bottom {
            return Ok(());
        }
        let width = right - area.x;
        // Each row of the request is padded as the server asks.
        let row_bytes = self.setup.image_format(width, 1).map_err(io::Error::other)?.stride as usize;
        let rows = x11::rows_per_put(&self.setup, row_bytes) as u32;
        let mut top = area.y;
        while top < bottom {
            let count = rows.min(bottom - top);
            let mut pixels = vec![0u8; row_bytes * count as usize];
            for (row, out) in pixels.chunks_exact_mut(row_bytes).enumerate() {
                let start = (top as usize + row) * stride + area.x as usize * bytes;
                let length = width as usize * bytes;
                out[..length].copy_from_slice(&self.canvas.pixels()[start..start + length]);
            }
            let request =
                x11::put_image(&self.setup, self.id, self.gc, width as u16, area.x as i16, top as i16, row_bytes, &pixels);
            self.send(&request)?;
            top += count;
        }
        Ok(())
    }
}

/// Saves a photo in the Pictures folder, named by the local time, and returns
/// its file name.
fn save(jpeg: &[u8]) -> io::Result<String> {
    fs::create_dir_all(PICTURES)?;
    // Alpine's `date` knows the time zone; the init's own clock is UTC.
    let stamp = alpine::run_line("date +%Y%m%d-%H%M%S")
        .ok()
        .map(|output| String::from_utf8_lossy(&output).trim().to_string())
        .filter(|stamp| !stamp.is_empty() && stamp.chars().all(|c| c.is_ascii_digit() || c == '-'))
        .unwrap_or_else(|| {
            let seconds = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_secs());
            seconds.to_string()
        });
    let mut name = format!("IMG_{stamp}.jpg");
    let mut copy = 2;
    while Path::new(PICTURES).join(&name).exists() {
        name = format!("IMG_{stamp}-{copy}.jpg");
        copy += 1;
    }
    fs::write(Path::new(PICTURES).join(&name), jpeg)?;
    log(&format!("camera window: saved {PICTURES}/{name}"));
    Ok(name)
}
