//! The touch panel: pages, buttons and the on-screen keyboard. It has no I/O: it takes
//! touches and status text, and returns requests, so it runs on the host in unit tests.

use crate::display::{Canvas, Rect, Rgb, ACCENT, BACKGROUND, FOREGROUND};
use crate::photo;
use crate::terminal::{key_bytes, Terminal};
use crate::wifi::{self, Credentials, Network};

const PANEL: Rgb = Rgb(28, 38, 54);
const BUTTON: Rgb = Rgb(44, 58, 80);
const PRESSED: Rgb = Rgb(240, 150, 60);
const MUTED: Rgb = Rgb(150, 165, 180);
const DANGER: Rgb = Rgb(170, 50, 50);

const TEXT_LARGE: u32 = 4;
const TEXT_MEDIUM: u32 = 3;
const TEXT_SMALL: u32 = 2;
const CHAR_WIDTH: u32 = 6;
const CHAR_HEIGHT: u32 = 7;
const MARGIN: u32 = 16;

const WIDTH: u32 = 480;
// Buttons leave space around them, so that a finger on the space presses nothing.
const BUTTON_GAP: u32 = 6;
const BOTTOM_ROW: Rect = Rect { x: MARGIN, y: 772, width: WIDTH - MARGIN * 2, height: 66 };
const FIRST_ACTION: Rect = Rect { x: 60, y: 200, width: 360, height: 88 };
const SECOND_ACTION: Rect = Rect { x: 60, y: 330, width: 360, height: 88 };
// Below both of the above, so that a double tap on one cannot press this.
const CONFIRM_ACTION: Rect = Rect { x: 60, y: 470, width: 360, height: 88 };
const NETWORK_ROWS: usize = 8;
const NETWORK_ROW_TOP: u32 = 106;
const NETWORK_ROW_STEP: u32 = 80;
const NETWORK_ROW_HEIGHT: u32 = 62;
const KEY_TOP: u32 = 440;
const KEY_HEIGHT: u32 = 80;

const PASSPHRASE_LIMIT: usize = 63;
const SENSOR_ROWS: usize = 4;
const SENSOR_ROW_TOP: u32 = 200;
const SENSOR_ROW_STEP: u32 = 72;
const SENSOR_ROW_HEIGHT: u32 = 58;
// Below every sensor row, so that a double tap on a row cannot confirm removing it.
const REMOVE_ACTION: Rect = Rect { x: 60, y: 600, width: 360, height: 88 };
/// Where the camera page shows its photo: a 3:4 portrait photo fills it.
const PHOTO_AREA: Rect = Rect { x: 0, y: 92, width: WIDTH, height: 640 };
const HEIGHT: u32 = 854;

// The console page: a header with two buttons, the terminal, and a keyboard of
// five short rows at the bottom, which can be hidden to show more lines.
const TERMINAL_LEFT: u32 = 6;
const TERMINAL_TOP: u32 = 52;
const TERMINAL_LINE: u32 = 18;
const TERMINAL_COLUMNS: usize = ((WIDTH - TERMINAL_LEFT * 2) / (CHAR_WIDTH * TEXT_SMALL)) as usize;
const TERMINAL_KEY_HEIGHT: u32 = 52;
const TERMINAL_KEYS_TOP: u32 = HEIGHT - 5 * TERMINAL_KEY_HEIGHT;
const HIDE_KEYS: Rect = Rect { x: 258, y: 2, width: 106, height: 40 };
const LEAVE_CONSOLE: Rect = Rect { x: 368, y: 2, width: 106, height: 40 };

const LOWER: [&str; 3] = ["qwertyuiop", "asdfghjkl", "zxcvbnm"];
const UPPER: [&str; 3] = ["QWERTYUIOP", "ASDFGHJKL", "ZXCVBNM"];
const SYMBOLS: [&str; 3] = ["1234567890", "!@#$%^&*()", "-_=+;:'"];
const MORE_SYMBOLS: [&str; 3] = ["[]{}<>/\\|~", ",.?\"`", ""];

/// Everything the home page shows, already as text.
#[derive(Default)]
pub struct Snapshot {
    pub address: String,
    pub network: String,
    pub signal: String,
    pub ssh: String,
    pub uptime: String,
    pub load: String,
    pub cpu_temperature: String,
    pub memory: String,
    pub disk: String,
    pub battery: String,
    pub battery_temperature: String,
    /// The scan's state in words, on the status page and the Bluetooth page.
    pub bluetooth: String,
    /// For the Bluetooth pages: the saved sensors, the sensors heard but not saved,
    /// and one line for each of the strongest devices.
    pub bluetooth_saved: Vec<SensorRow>,
    pub bluetooth_candidates: Vec<SensorRow>,
    pub bluetooth_nearby: Vec<String>,
}

/// A sensor as one row of the panel: its address, and the text to show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SensorRow {
    pub address: String,
    pub line: String,
    /// False for a sensor whose values are encrypted, which cannot be saved.
    pub readable: bool,
}

/// Work the panel cannot do itself, because it would stop the screen responding.
pub enum Request {
    Restart,
    PowerOff,
    Scan,
    Join(Credentials),
    Forget,
    /// Turns the BLE scan on or off, and keeps that choice.
    Bluetooth(bool),
    /// Saves or removes a sensor, by its address as text.
    AddSensor(String),
    RemoveSensor(String),
    /// Takes a photo with the named camera, "rear" or "front".
    Photo(&'static str),
    /// Shows live video from the named camera on the camera page, in place of
    /// any live video already shown.
    Live(&'static str),
    StopLive,
    /// Starts the console's shell if it is not running, and gives it this size.
    TerminalSize { columns: usize, rows: usize },
    /// What was typed on the console page, for the shell.
    TerminalInput(Vec<u8>),
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Layer {
    Lower,
    Upper,
    Symbols,
    MoreSymbols,
}

impl Layer {
    fn rows(self) -> [&'static str; 3] {
        match self {
            Layer::Lower => LOWER,
            Layer::Upper => UPPER,
            Layer::Symbols => SYMBOLS,
            Layer::MoreSymbols => MORE_SYMBOLS,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Page {
    Home,
    Power,
    ConfirmRestart,
    ConfirmPowerOff,
    ConfirmForget,
    Networks,
    Keyboard,
    Bluetooth,
    AddSensor,
    ConfirmRemoveSensor,
    Camera,
    Console,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Press {
    OpenNetworks,
    OpenPower,
    Home,
    AskRestart,
    AskPowerOff,
    Restart,
    PowerOff,
    Rescan,
    AskForget,
    Forget,
    OpenNetworksList,
    Pick(usize),
    Key(char),
    Delete,
    Shift,
    SwitchSymbols,
    ToggleShow,
    Join,
    CancelKeyboard,
    OpenBluetooth,
    BluetoothOn,
    BluetoothOff,
    OpenAddSensor,
    PickSaved(usize),
    PickCandidate(usize),
    RemoveSensor,
    OpenCamera,
    Photo(&'static str),
    ToggleLive,
    OpenConsole,
    /// Hides or shows the console's keyboard.
    ToggleConsoleKeys,
    /// The console's own keys: Ctrl for the next key, Esc, Tab, Enter, and the
    /// arrows by the letter of their escape sequence (A up, B down, C right, D left).
    Control,
    Escape,
    Tab,
    Enter,
    Arrow(u8),
}

struct Button {
    area: Rect,
    label: String,
    press: Press,
    color: Rgb,
    scale: u32,
}

impl Button {
    fn new(area: Rect, label: &str, press: Press) -> Self {
        Self { area, label: label.to_string(), press, color: BUTTON, scale: TEXT_MEDIUM }
    }

    fn danger(mut self) -> Self {
        self.color = DANGER;
        self
    }
}

/// An image made small enough for the camera page.
pub fn fit_photo(image: &photo::Rgb) -> photo::Rgb {
    photo::fit(image, PHOTO_AREA.width as usize, PHOTO_AREA.height as usize)
}

/// One of `parts` equal buttons side by side, with a gap between them.
fn split(area: Rect, parts: u32, index: u32) -> Rect {
    let each = area.width / parts;
    Rect {
        x: area.x + each * index + BUTTON_GAP / 2,
        y: area.y,
        width: each - BUTTON_GAP,
        height: area.height,
    }
}

pub struct Ui {
    page: Page,
    /// The button under the finger when it came down, and where the finger last was.
    pressed: Option<Press>,
    finger_down: bool,
    position: (i32, i32),
    networks: Vec<Network>,
    message: String,
    /// The chosen network, and what has been typed. Cleared when leaving the page.
    target: Option<Vec<u8>>,
    typed: String,
    show_typed: bool,
    layer: Layer,
    /// The sensor lists as last drawn, so that a tap picks what was on the screen.
    saved: Vec<SensorRow>,
    candidates: Vec<SensorRow>,
    /// The address of the sensor being removed.
    sensor_target: Option<String>,
    /// The last photo or live frame, already sized for the camera page.
    photo: Option<photo::Rgb>,
    /// The camera showing live video, if any.
    live: Option<&'static str>,
    /// The console page: the shell's output, whether the keyboard shows, whether
    /// Ctrl is held for the next key, how many lines the view is scrolled back,
    /// and while a finger drags the text, where it started and the scroll then.
    terminal: Terminal,
    console_keys: bool,
    control: bool,
    console_scroll: usize,
    drag: Option<(i32, usize)>,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            page: Page::Home,
            pressed: None,
            finger_down: false,
            position: (0, 0),
            networks: Vec::new(),
            message: String::new(),
            target: None,
            typed: String::new(),
            show_typed: false,
            layer: Layer::Lower,
            saved: Vec::new(),
            candidates: Vec::new(),
            sensor_target: None,
            photo: None,
            live: None,
            terminal: Terminal::new(TERMINAL_COLUMNS, terminal_rows(true)),
            console_keys: true,
            control: false,
            console_scroll: 0,
            drag: None,
        }
    }
}

/// How many lines the console shows, with or without the keyboard.
fn terminal_rows(keys: bool) -> usize {
    let bottom = if keys { TERMINAL_KEYS_TOP } else { HEIGHT };
    ((bottom - 6 - TERMINAL_TOP) / TERMINAL_LINE) as usize
}

impl Ui {
    pub fn set_networks(&mut self, networks: Vec<Network>) {
        self.networks = networks;
    }

    /// Takes the sensor lists that the next drawing will show.
    pub fn set_sensors(&mut self, saved: Vec<SensorRow>, candidates: Vec<SensorRow>) {
        self.saved = saved;
        self.candidates = candidates;
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    /// Shows a photo on the camera page, made smaller to fit it.
    pub fn set_photo(&mut self, image: &photo::Rgb) {
        self.photo = Some(fit_photo(image));
    }

    /// Shows a live frame already made to fit by `fit_photo`, unless live video
    /// has stopped since it was taken.
    pub fn show_live(&mut self, image: photo::Rgb) {
        if self.live.is_some() {
            self.photo = Some(image);
        }
    }

    /// Live video stopped on its own, for example at its time limit or when the
    /// camera was busy. The last frame stays.
    pub fn live_ended(&mut self, message: String) {
        self.live = None;
        self.message = message;
    }

    /// Output from the console's shell. A view scrolled back stays on the same lines.
    pub fn terminal_output(&mut self, bytes: &[u8]) {
        let added = self.terminal.feed(bytes);
        if self.console_scroll > 0 {
            self.console_scroll = (self.console_scroll + added).min(self.max_scroll());
        }
    }

    /// A note from this system on the console, such as that the shell ended.
    pub fn terminal_note(&mut self, text: &str) {
        self.terminal.note(text);
        self.console_scroll = 0;
    }

    /// The furthest the console can scroll back: to its first line at the top.
    fn max_scroll(&self) -> usize {
        self.terminal.lines().len().saturating_sub(self.terminal.rows())
    }

    /// The console's columns and rows now.
    pub fn terminal_dimensions(&self) -> (usize, usize) {
        (self.terminal.columns(), self.terminal.rows())
    }

    /// The console's size request, for when it opens or the keyboard changes.
    fn terminal_size(&self) -> Request {
        Request::TerminalSize { columns: self.terminal.columns(), rows: self.terminal.rows() }
    }

    /// Bytes for the shell; typing also brings the view back to the end.
    fn send(&mut self, bytes: Vec<u8>) -> Option<Request> {
        self.control = false;
        self.console_scroll = 0;
        Some(Request::TerminalInput(bytes))
    }

    /// Back to the home page, without anything typed. Used when the screen turns off,
    /// so that a passphrase does not stay on the panel.
    pub fn reset(&mut self) {
        self.page = Page::Home;
        self.pressed = None;
        self.finger_down = false;
        self.sensor_target = None;
        // A photo should not wait on a dark screen for the next person to wake it.
        self.photo = None;
        self.live = None;
        self.clear_typed();
    }

    fn clear_typed(&mut self) {
        self.typed.clear();
        self.target = None;
        self.show_typed = false;
        self.layer = Layer::Lower;
    }

    /// Takes the finger's position, or `None` when it lifts. A button acts when the
    /// finger lifts over it, so that sliding off cancels the press.
    pub fn touch(&mut self, position: Option<(i32, i32)>) -> Option<Request> {
        match position {
            Some(position) => {
                if !self.finger_down {
                    self.finger_down = true;
                    self.pressed = self.press_at(position);
                    // On the console's text, a finger drags the view instead.
                    let text_bottom = if self.console_keys { TERMINAL_KEYS_TOP } else { HEIGHT };
                    if self.page == Page::Console
                        && self.pressed.is_none()
                        && position.1 >= TERMINAL_TOP as i32 - 6
                        && position.1 < text_bottom as i32
                    {
                        self.drag = Some((position.1, self.console_scroll));
                    }
                }
                if let Some((start, scroll)) = self.drag {
                    // Pulling the text down shows earlier lines.
                    let lines = (position.1 - start) / TERMINAL_LINE as i32;
                    let wanted = scroll as i64 + i64::from(lines);
                    self.console_scroll = wanted.clamp(0, self.max_scroll() as i64) as usize;
                }
                self.position = position;
                None
            }
            None => {
                self.finger_down = false;
                self.drag = None;
                let pressed = self.pressed.take()?;
                if self.press_at(self.position) == Some(pressed) {
                    self.act(pressed)
                } else {
                    None
                }
            }
        }
    }

    fn press_at(&self, (x, y): (i32, i32)) -> Option<Press> {
        self.buttons().iter().find(|button| button.area.contains(x, y)).map(|button| button.press)
    }

    fn act(&mut self, press: Press) -> Option<Request> {
        // A message belongs to the page it was shown on. A slow reply can still arrive
        // later, which is how a result from another page reaches the screen.
        let page = self.page;
        let request = self.apply(press);
        // Live video only runs while its page shows.
        if page == Page::Camera && self.page != Page::Camera && self.live.take().is_some() {
            self.message.clear();
            return Some(Request::StopLive);
        }
        if self.page != page && request.is_none() {
            self.message.clear();
        }
        request
    }

    fn apply(&mut self, press: Press) -> Option<Request> {
        match press {
            Press::OpenNetworks => {
                self.page = Page::Networks;
                self.message = "Scanning...".to_string();
                return Some(Request::Scan);
            }
            Press::OpenPower => self.page = Page::Power,
            Press::OpenBluetooth => self.page = Page::Bluetooth,
            Press::OpenCamera => self.page = Page::Camera,
            // During live video, REAR and FRONT switch cameras instead.
            Press::Photo(camera) if self.live.is_some() => {
                if self.live != Some(camera) {
                    self.live = Some(camera);
                    self.photo = None;
                    self.message = format!("Live: {camera}");
                    return Some(Request::Live(camera));
                }
            }
            Press::Photo(camera) => {
                self.message = "Taking a photo...".to_string();
                return Some(Request::Photo(camera));
            }
            Press::ToggleLive => {
                if self.live.take().is_some() {
                    self.message = "Live video stopped".to_string();
                    return Some(Request::StopLive);
                }
                self.live = Some("rear");
                self.photo = None;
                self.message = "Live: rear".to_string();
                return Some(Request::Live("rear"));
            }
            Press::BluetoothOn => {
                self.message = "Turning on...".to_string();
                return Some(Request::Bluetooth(true));
            }
            Press::BluetoothOff => {
                self.message = "Turning off...".to_string();
                return Some(Request::Bluetooth(false));
            }
            Press::OpenAddSensor => self.page = Page::AddSensor,
            Press::PickCandidate(index) => {
                if self.candidates.get(index).is_some_and(|row| !row.readable) {
                    // Short enough for one line. The row shows which sensor it is.
                    self.message = "Encrypted, so it cannot be read.".to_string();
                } else if let Some(row) = self.candidates.get(index) {
                    self.page = Page::Bluetooth;
                    self.message = "Adding...".to_string();
                    return Some(Request::AddSensor(row.address.clone()));
                }
            }
            Press::PickSaved(index) => {
                if let Some(row) = self.saved.get(index) {
                    self.sensor_target = Some(row.address.clone());
                    self.page = Page::ConfirmRemoveSensor;
                }
            }
            Press::RemoveSensor => {
                self.page = Page::Bluetooth;
                if let Some(address) = self.sensor_target.take() {
                    self.message = "Removing...".to_string();
                    return Some(Request::RemoveSensor(address));
                }
            }
            Press::Home => self.page = Page::Home,
            Press::AskRestart => self.page = Page::ConfirmRestart,
            Press::AskPowerOff => self.page = Page::ConfirmPowerOff,
            Press::Restart => {
                self.message = "Restarting...".to_string();
                return Some(Request::Restart);
            }
            Press::PowerOff => {
                self.message = "Powering off...".to_string();
                return Some(Request::PowerOff);
            }
            Press::Rescan => {
                self.message = "Scanning...".to_string();
                return Some(Request::Scan);
            }
            Press::AskForget => self.page = Page::ConfirmForget,
            Press::OpenNetworksList => self.page = Page::Networks,
            Press::Forget => {
                self.page = Page::Networks;
                self.message = "Forgetting...".to_string();
                return Some(Request::Forget);
            }
            Press::Pick(index) => {
                if let Some(network) = self.networks.get(index) {
                    self.target = Some(network.ssid.clone());
                    self.typed.clear();
                    self.message.clear();
                    self.page = Page::Keyboard;
                }
            }
            Press::OpenConsole => {
                self.page = Page::Console;
                return Some(self.terminal_size());
            }
            Press::ToggleConsoleKeys => {
                self.console_keys = !self.console_keys;
                self.terminal.resize(TERMINAL_COLUMNS, terminal_rows(self.console_keys));
                self.console_scroll = self.console_scroll.min(self.max_scroll());
                return Some(self.terminal_size());
            }
            Press::Control => self.control = !self.control,
            Press::Escape => return self.send(vec![0x1B]),
            Press::Tab => return self.send(vec![b'\t']),
            Press::Enter => return self.send(vec![b'\r']),
            Press::Arrow(letter) => return self.send(vec![0x1B, b'[', letter]),
            Press::Key(character) if self.page == Page::Console => {
                let bytes = key_bytes(character, self.control);
                return self.send(bytes);
            }
            Press::Delete if self.page == Page::Console => return self.send(vec![0x7F]),
            Press::Key(character) => {
                if self.typed.len() < PASSPHRASE_LIMIT {
                    self.typed.push(character);
                }
                self.message.clear();
            }
            Press::Delete => {
                self.typed.pop();
            }
            Press::Shift => {
                self.layer = match self.layer {
                    Layer::Lower => Layer::Upper,
                    Layer::Upper => Layer::Lower,
                    Layer::Symbols => Layer::MoreSymbols,
                    Layer::MoreSymbols => Layer::Symbols,
                }
            }
            Press::SwitchSymbols => {
                self.layer = match self.layer {
                    Layer::Lower | Layer::Upper => Layer::Symbols,
                    Layer::Symbols | Layer::MoreSymbols => Layer::Lower,
                }
            }
            Press::ToggleShow => self.show_typed = !self.show_typed,
            Press::CancelKeyboard => {
                self.clear_typed();
                self.page = Page::Networks;
            }
            Press::Join => {
                let ssid = self.target.as_deref().unwrap_or_default();
                match wifi::credentials(ssid, &self.typed) {
                    Ok(credentials) => {
                        self.clear_typed();
                        self.page = Page::Networks;
                        self.message = "Joining...".to_string();
                        return Some(Request::Join(credentials));
                    }
                    Err(err) => self.message = err.to_string(),
                }
            }
        }
        None
    }

    fn buttons(&self) -> Vec<Button> {
        match self.page {
            // Five to a row only fit at the small size, and BLUETOOTH not even then.
            Page::Home => [
                ("WI-FI", Press::OpenNetworks),
                ("BLE", Press::OpenBluetooth),
                ("CAMERA", Press::OpenCamera),
                ("CONSOLE", Press::OpenConsole),
                ("POWER", Press::OpenPower),
            ]
            .into_iter()
            .enumerate()
            .map(|(index, (label, press))| Button {
                scale: TEXT_SMALL,
                ..Button::new(split(BOTTOM_ROW, 5, index as u32), label, press)
            })
            .collect(),
            Page::Console => self.console_buttons(),
            Page::Camera => {
                let mut live = Button::new(split(BOTTOM_ROW, 4, 2), "LIVE", Press::ToggleLive);
                if self.live.is_some() {
                    live.label = "STOP".to_string();
                    live.color = DANGER;
                }
                vec![
                    Button::new(split(BOTTOM_ROW, 4, 0), "REAR", Press::Photo("rear")),
                    Button::new(split(BOTTOM_ROW, 4, 1), "FRONT", Press::Photo("front")),
                    live,
                    Button::new(split(BOTTOM_ROW, 4, 3), "BACK", Press::Home),
                ]
            }
            Page::Power => vec![
                Button::new(FIRST_ACTION, "RESTART", Press::AskRestart),
                Button::new(SECOND_ACTION, "POWER OFF", Press::AskPowerOff)
                    .danger(),
                Button::new(BOTTOM_ROW, "BACK", Press::Home),
            ],
            Page::ConfirmRestart => vec![
                Button::new(CONFIRM_ACTION, "YES, RESTART", Press::Restart)
                    .danger(),
                Button::new(BOTTOM_ROW, "CANCEL", Press::OpenPower),
            ],
            Page::ConfirmPowerOff => vec![
                Button::new(CONFIRM_ACTION, "YES, POWER OFF", Press::PowerOff)
                    .danger(),
                Button::new(BOTTOM_ROW, "CANCEL", Press::OpenPower),
            ],
            Page::ConfirmForget => vec![
                Button::new(CONFIRM_ACTION, "YES, FORGET", Press::Forget)
                    .danger(),
                Button::new(BOTTOM_ROW, "CANCEL", Press::OpenNetworksList),
            ],
            Page::Networks => {
                let mut buttons: Vec<Button> = self
                    .networks
                    .iter()
                    .take(NETWORK_ROWS)
                    .enumerate()
                    .map(|(index, network)| {
                        let area = Rect {
                            x: MARGIN,
                            y: NETWORK_ROW_TOP + index as u32 * NETWORK_ROW_STEP,
                            width: WIDTH - MARGIN * 2,
                            height: NETWORK_ROW_HEIGHT,
                        };
                        let mut button = Button::new(area, &network.label(), Press::Pick(index));
                        button.scale = TEXT_SMALL;
                        button
                    })
                    .collect();
                buttons.push(Button::new(split(BOTTOM_ROW, 3, 0), "SCAN", Press::Rescan));
                buttons.push(Button::new(split(BOTTOM_ROW, 3, 1), "FORGET", Press::AskForget).danger());
                buttons.push(Button::new(split(BOTTOM_ROW, 3, 2), "BACK", Press::Home));
                buttons
            }
            Page::Keyboard => self.keyboard_buttons(),
            Page::Bluetooth => {
                let mut buttons: Vec<Button> = self
                    .saved
                    .iter()
                    .take(SENSOR_ROWS)
                    .enumerate()
                    .map(|(index, row)| {
                        let area = Rect {
                            x: MARGIN,
                            y: SENSOR_ROW_TOP + index as u32 * SENSOR_ROW_STEP,
                            width: WIDTH - MARGIN * 2,
                            height: SENSOR_ROW_HEIGHT,
                        };
                        let mut button = Button::new(area, &row.line, Press::PickSaved(index));
                        button.scale = TEXT_SMALL;
                        button
                    })
                    .collect();
                buttons.push(Button::new(split(BOTTOM_ROW, 4, 0), "ON", Press::BluetoothOn));
                buttons.push(Button::new(split(BOTTOM_ROW, 4, 1), "OFF", Press::BluetoothOff));
                buttons.push(Button::new(split(BOTTOM_ROW, 4, 2), "ADD", Press::OpenAddSensor));
                buttons.push(Button::new(split(BOTTOM_ROW, 4, 3), "BACK", Press::Home));
                buttons
            }
            Page::AddSensor => {
                let mut buttons: Vec<Button> = self
                    .candidates
                    .iter()
                    .take(NETWORK_ROWS)
                    .enumerate()
                    .map(|(index, row)| {
                        let area = Rect {
                            x: MARGIN,
                            y: NETWORK_ROW_TOP + index as u32 * NETWORK_ROW_STEP,
                            width: WIDTH - MARGIN * 2,
                            height: NETWORK_ROW_HEIGHT,
                        };
                        let mut button = Button::new(area, &row.line, Press::PickCandidate(index));
                        button.scale = TEXT_SMALL;
                        button
                    })
                    .collect();
                buttons.push(Button::new(BOTTOM_ROW, "BACK", Press::OpenBluetooth));
                buttons
            }
            Page::ConfirmRemoveSensor => vec![
                Button::new(REMOVE_ACTION, "YES, REMOVE", Press::RemoveSensor).danger(),
                Button::new(BOTTOM_ROW, "CANCEL", Press::OpenBluetooth),
            ],
        }
    }

    fn console_buttons(&self) -> Vec<Button> {
        let small = |area: Rect, label: &str, press: Press| Button { scale: TEXT_SMALL, ..Button::new(area, label, press) };
        let mut buttons = vec![
            small(HIDE_KEYS, if self.console_keys { "HIDE KEYS" } else { "KEYS" }, Press::ToggleConsoleKeys),
            Button::new(LEAVE_CONSOLE, "BACK", Press::Home),
        ];
        if !self.console_keys {
            return buttons;
        }
        let row = |index: u32| Rect {
            x: 0,
            y: TERMINAL_KEYS_TOP + index * TERMINAL_KEY_HEIGHT,
            width: WIDTH,
            height: TERMINAL_KEY_HEIGHT,
        };
        // The arrows as letters, since the font is ASCII.
        let special = [
            ("CTRL", Press::Control),
            ("ESC", Press::Escape),
            ("TAB", Press::Tab),
            ("<", Press::Arrow(b'D')),
            ("^", Press::Arrow(b'A')),
            ("v", Press::Arrow(b'B')),
            (">", Press::Arrow(b'C')),
        ];
        for (index, (label, press)) in special.into_iter().enumerate() {
            let mut button = small(split(row(0), 7, index as u32), label, press);
            if press == Press::Control && self.control {
                button.color = PRESSED;
            }
            buttons.push(button);
        }
        buttons.extend(self.key_rows(row(1).y, TERMINAL_KEY_HEIGHT));
        let last = row(4);
        let symbols = if matches!(self.layer, Layer::Lower | Layer::Upper) { "123" } else { "abc" };
        buttons.push(Button::new(split(last, 4, 0), symbols, Press::SwitchSymbols));
        let space = Rect { width: split(last, 4, 1).width * 2 + BUTTON_GAP, ..split(last, 4, 1) };
        buttons.push(Button::new(space, "SPACE", Press::Key(' ')));
        buttons.push(Button::new(split(last, 4, 3), "ENTER", Press::Enter));
        buttons
    }

    fn keyboard_buttons(&self) -> Vec<Button> {
        let mut buttons = self.key_rows(KEY_TOP, KEY_HEIGHT);
        let y = KEY_TOP + 3 * KEY_HEIGHT;
        let row = Rect { x: 0, y, width: WIDTH, height: KEY_HEIGHT };
        let symbols = if matches!(self.layer, Layer::Lower | Layer::Upper) { "123" } else { "abc" };
        buttons.push(Button::new(split(row, 4, 0), symbols, Press::SwitchSymbols));
        buttons.push(Button::new(split(row, 4, 1), "SPACE", Press::Key(' ')));
        buttons.push(Button::new(
            split(row, 4, 2),
            if self.show_typed { "HIDE" } else { "SHOW" },
            Press::ToggleShow,
        ));
        buttons.push(Button::new(split(row, 4, 3), "JOIN", Press::Join));
        buttons.push(Button::new(
            BOTTOM_ROW,
            "CANCEL",
            Press::CancelKeyboard,
        ));
        buttons
    }

    /// The three rows of character keys, with shift and delete, from `top`.
    fn key_rows(&self, top: u32, height: u32) -> Vec<Button> {
        const KEY_WIDTH: u32 = WIDTH / 10;
        let mut buttons = Vec::new();
        let rows = self.layer.rows();
        for (row, keys) in rows.iter().enumerate() {
            let count = keys.chars().count() as u32;
            let y = top + row as u32 * height;
            let mut left = (WIDTH - count * KEY_WIDTH) / 2;
            if row == 2 {
                buttons.push(Button::new(
                    Rect { x: 0, y, width: left.max(KEY_WIDTH), height },
                    match self.layer {
                        Layer::Lower => "abc",
                        Layer::Upper => "ABC",
                        Layer::Symbols => "1/2",
                        Layer::MoreSymbols => "2/2",
                    },
                    Press::Shift,
                ));
                left = left.max(KEY_WIDTH);
            }
            for (index, key) in keys.chars().enumerate() {
                buttons.push(Button::new(
                    Rect { x: left + index as u32 * KEY_WIDTH, y, width: KEY_WIDTH, height },
                    &key.to_string(),
                    Press::Key(key),
                ));
            }
            if row == 2 {
                let x = left + count * KEY_WIDTH;
                buttons.push(Button::new(
                    Rect { x, y, width: WIDTH - x, height },
                    "DEL",
                    Press::Delete,
                ));
            }
        }
        buttons
    }

    /// The typed text, showing only its last character unless asked to show it all.
    fn typed_text(&self) -> String {
        if self.show_typed {
            return self.typed.clone();
        }
        let count = self.typed.chars().count();
        let last = self.typed.chars().last().map(String::from).unwrap_or_default();
        format!("{}{last}", "*".repeat(count.saturating_sub(1)))
    }

    pub fn render(&self, canvas: &mut Canvas, status: &Snapshot) {
        canvas.clear(BACKGROUND);
        match self.page {
            Page::Home => self.render_home(canvas, status),
            Page::Power => title(canvas, "POWER"),
            Page::ConfirmRestart => {
                title(canvas, "RESTART?");
                canvas.draw_text(MARGIN, 200, TEXT_MEDIUM, "SSH will drop for", FOREGROUND);
                canvas.draw_text(MARGIN, 240, TEXT_MEDIUM, "about 30 seconds.", FOREGROUND);
            }
            Page::ConfirmPowerOff => {
                title(canvas, "POWER OFF?");
                canvas.draw_text(MARGIN, 200, TEXT_MEDIUM, "With USB attached it", FOREGROUND);
                canvas.draw_text(MARGIN, 240, TEXT_MEDIUM, "starts again by itself.", FOREGROUND);
            }
            Page::ConfirmForget => {
                title(canvas, "FORGET WI-FI?");
                canvas.draw_text(MARGIN, 200, TEXT_MEDIUM, "It leaves the network,", FOREGROUND);
                canvas.draw_text(MARGIN, 240, TEXT_MEDIUM, "and SSH stops working", FOREGROUND);
                canvas.draw_text(MARGIN, 280, TEXT_MEDIUM, "until it joins again.", FOREGROUND);
            }
            Page::Networks => title(canvas, "WI-FI"),
            Page::Keyboard => self.render_keyboard_top(canvas),
            Page::Bluetooth => render_bluetooth(canvas, status, self.saved.is_empty()),
            Page::AddSensor => {
                title(canvas, "ADD SENSOR");
                if self.candidates.is_empty() {
                    canvas.draw_text(MARGIN, 120, TEXT_SMALL, "No new sensors heard.", FOREGROUND);
                    canvas.draw_text(MARGIN, 150, TEXT_SMALL, "Turn the scan on and wait.", MUTED);
                }
            }
            Page::ConfirmRemoveSensor => {
                title(canvas, "REMOVE SENSOR?");
                let address = self.sensor_target.as_deref().unwrap_or_default();
                canvas.draw_text(MARGIN, 200, TEXT_MEDIUM, address, FOREGROUND);
                canvas.draw_text(MARGIN, 260, TEXT_MEDIUM, "It is no longer shown", FOREGROUND);
                canvas.draw_text(MARGIN, 300, TEXT_MEDIUM, "or recorded.", FOREGROUND);
            }
            Page::Console => self.render_console(canvas),
            Page::Camera => {
                title(canvas, "CAMERA");
                match &self.photo {
                    Some(image) => {
                        let (width, height) = (image.width as u32, image.height as u32);
                        let x = PHOTO_AREA.x + PHOTO_AREA.width.saturating_sub(width) / 2;
                        let y = PHOTO_AREA.y + PHOTO_AREA.height.saturating_sub(height) / 2;
                        canvas.draw_rgb(x, y, width, height, &image.data);
                    }
                    None if self.live.is_some() => {
                        canvas.draw_text(MARGIN, 200, TEXT_MEDIUM, "Starting the camera...", FOREGROUND);
                    }
                    None => {
                        canvas.draw_text(MARGIN, 200, TEXT_MEDIUM, "Tap REAR or FRONT", FOREGROUND);
                        canvas.draw_text(MARGIN, 240, TEXT_MEDIUM, "to take a photo, or", FOREGROUND);
                        canvas.draw_text(MARGIN, 280, TEXT_MEDIUM, "LIVE for video.", FOREGROUND);
                        canvas.draw_text(MARGIN, 340, TEXT_SMALL, "Nothing is saved, and the", MUTED);
                        canvas.draw_text(MARGIN, 366, TEXT_SMALL, "image is cleared when the", MUTED);
                        canvas.draw_text(MARGIN, 392, TEXT_SMALL, "screen turns off.", MUTED);
                    }
                }
            }
        }
        for button in self.buttons() {
            let color = if self.pressed == Some(button.press) { PRESSED } else { button.color };
            canvas.fill(button.area.inset(2), color);
            canvas.draw_text_centered(button.area, button.scale, &button.label, FOREGROUND);
        }
        if !self.message.is_empty() && !matches!(self.page, Page::Home | Page::Console) {
            let y = if self.page == Page::Keyboard { 400 } else { 76 };
            canvas.draw_text(MARGIN, y, TEXT_SMALL, &self.message, ACCENT);
        }
    }

    fn render_home(&self, canvas: &mut Canvas, status: &Snapshot) {
        title(canvas, "AMAZING A32");
        let groups: [(&str, u32, u32, &[(&str, &str)]); 3] = [
            (
                "NETWORK",
                88,
                206,
                &[
                    ("ADDR", &status.address),
                    ("WI-FI", &status.network),
                    ("SIGNAL", &status.signal),
                    ("SSH", &status.ssh),
                ],
            ),
            (
                "SYSTEM",
                310,
                248,
                &[
                    ("UP", &status.uptime),
                    ("LOAD", &status.load),
                    ("CPU", &status.cpu_temperature),
                    ("MEM", &status.memory),
                    ("DISK", &status.disk),
                ],
            ),
            (
                "BATTERY",
                574,
                170,
                &[("LEVEL", &status.battery), ("TEMP", &status.battery_temperature)],
            ),
        ];
        for (name, top, height, rows) in groups {
            canvas.fill(Rect { x: 0, y: top, width: WIDTH, height }, PANEL);
            canvas.draw_text(MARGIN, top + 12, TEXT_SMALL, name, ACCENT);
            for (index, (label, value)) in rows.iter().enumerate() {
                let y = top + 42 + index as u32 * 40;
                canvas.draw_text(MARGIN, y + 4, TEXT_SMALL, label, MUTED);
                canvas.draw_text(100, y, TEXT_MEDIUM, value, FOREGROUND);
            }
        }
    }

    fn render_console(&self, canvas: &mut Canvas) {
        canvas.draw_text(MARGIN, 12, TEXT_MEDIUM, "CONSOLE", ACCENT);
        if self.console_scroll > 0 {
            canvas.draw_text(160, 16, TEXT_SMALL, &format!("{} up", self.console_scroll), MUTED);
        }
        let rows = self.terminal.rows();
        let height = rows as u32 * TERMINAL_LINE + 8;
        canvas.fill(Rect { x: 0, y: TERMINAL_TOP - 6, width: WIDTH, height }, PANEL);
        let lines = self.terminal.lines();
        let end = lines.len().saturating_sub(self.console_scroll);
        let start = end.saturating_sub(rows);
        for (index, line) in lines[start..end].iter().enumerate() {
            let text: String = line.iter().collect();
            let y = TERMINAL_TOP + index as u32 * TERMINAL_LINE;
            canvas.draw_text(TERMINAL_LEFT, y, TEXT_SMALL, &text, FOREGROUND);
        }
        // The cursor, as a bar under its character, while it is in view.
        let (row, column) = self.terminal.cursor();
        if (start..end).contains(&row) {
            let x = TERMINAL_LEFT + column as u32 * CHAR_WIDTH * TEXT_SMALL;
            let y = TERMINAL_TOP + (row - start) as u32 * TERMINAL_LINE + CHAR_HEIGHT * TEXT_SMALL + 1;
            canvas.fill(Rect { x, y, width: (CHAR_WIDTH - 1) * TEXT_SMALL, height: 2 }, ACCENT);
        }
    }

    fn render_keyboard_top(&self, canvas: &mut Canvas) {
        title(canvas, "PASSPHRASE");
        let name = self.target.as_deref().map(wifi::display_ssid).unwrap_or_default();
        canvas.draw_text(MARGIN, 96, TEXT_SMALL, &name, MUTED);
        canvas.fill(Rect { x: MARGIN, y: 150, width: WIDTH - MARGIN * 2, height: 200 }, PANEL);
        // Up to 3 lines at this size, which holds the longest allowed passphrase.
        let columns = ((WIDTH - MARGIN * 4) / (CHAR_WIDTH * TEXT_MEDIUM)) as usize;
        let text: Vec<char> = self.typed_text().chars().collect();
        for (index, line) in text.chunks(columns).enumerate() {
            let line: String = line.iter().collect();
            let y = 170 + index as u32 * (CHAR_HEIGHT * TEXT_MEDIUM + 18);
            canvas.draw_text(MARGIN * 2, y, TEXT_MEDIUM, &line, FOREGROUND);
        }
    }
}

/// The scan's state and the strongest devices, as many as fit. The saved sensors
/// are buttons, drawn with the other buttons.
fn render_bluetooth(canvas: &mut Canvas, status: &Snapshot, none_saved: bool) {
    const NEARBY_TOP: u32 = 500;
    const NEARBY_STEP: u32 = 34;
    title(canvas, "BLUETOOTH");
    let state = if status.bluetooth.is_empty() { "off" } else { status.bluetooth.as_str() };
    canvas.draw_text(MARGIN, 110, TEXT_SMALL, state, FOREGROUND);
    canvas.draw_text(MARGIN, 136, TEXT_SMALL, "Scanning slows Wi-Fi.", MUTED);

    canvas.draw_text(MARGIN, 172, TEXT_SMALL, "MY SENSORS, TAP ONE TO REMOVE", ACCENT);
    if none_saved {
        canvas.draw_text(MARGIN, 214, TEXT_SMALL, "None saved. Press ADD.", MUTED);
    }

    canvas.draw_text(MARGIN, NEARBY_TOP, TEXT_SMALL, "STRONGEST", ACCENT);
    let rows = ((BOTTOM_ROW.y - NEARBY_TOP - 40) / NEARBY_STEP) as usize;
    for (index, line) in status.bluetooth_nearby.iter().take(rows).enumerate() {
        canvas.draw_text(MARGIN, NEARBY_TOP + 34 + index as u32 * NEARBY_STEP, TEXT_SMALL, line, FOREGROUND);
    }
}

fn title(canvas: &mut Canvas, text: &str) {
    canvas.draw_text(MARGIN, 26, TEXT_LARGE, text, ACCENT);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network(name: &str) -> Network {
        Network { ssid: name.as_bytes().to_vec(), signal: -40 }
    }

    fn tap(ui: &mut Ui, x: i32, y: i32) -> Option<Request> {
        ui.touch(Some((x, y)));
        ui.touch(None)
    }

    fn tap_button(ui: &mut Ui, press: Press) -> Option<Request> {
        let button = ui
            .buttons()
            .into_iter()
            .find(|button| button.press == press)
            .unwrap_or_else(|| panic!("no button {press:?} on {:?}", ui.page));
        tap(ui, (button.area.x + 2) as i32, (button.area.y + 2) as i32)
    }

    fn type_text(ui: &mut Ui, text: &str) {
        for character in text.chars() {
            let target = [Layer::Lower, Layer::Upper, Layer::Symbols, Layer::MoreSymbols]
                .into_iter()
                .find(|layer| character == ' ' || layer.rows().concat().contains(character))
                .unwrap_or_else(|| panic!("no layer has {character:?}"));
            while ui.layer != target {
                let same_group = matches!(
                    (ui.layer, target),
                    (Layer::Lower | Layer::Upper, Layer::Lower | Layer::Upper)
                        | (Layer::Symbols | Layer::MoreSymbols, Layer::Symbols | Layer::MoreSymbols)
                );
                tap_button(ui, if same_group { Press::Shift } else { Press::SwitchSymbols });
            }
            tap_button(ui, Press::Key(character));
        }
    }

    #[test]
    fn every_printable_character_can_be_typed() {
        let mut ui = Ui { page: Page::Keyboard, ..Ui::default() };
        let all: String = (0x20u8..=0x7e).map(char::from).collect();
        type_text(&mut ui, &all[..PASSPHRASE_LIMIT]);
        assert_eq!(ui.typed, all[..PASSPHRASE_LIMIT]);
        ui.typed.clear();
        type_text(&mut ui, &all[PASSPHRASE_LIMIT..]);
        assert_eq!(ui.typed, all[PASSPHRASE_LIMIT..]);
    }

    #[test]
    fn a_button_acts_on_release_over_it() {
        let mut ui = Ui::default();
        ui.touch(Some((100, 800)));
        assert_eq!(ui.page, Page::Home);
        assert!(matches!(ui.touch(None), Some(Request::Scan)));
        assert_eq!(ui.page, Page::Networks);
    }

    #[test]
    fn sliding_off_a_button_cancels_it() {
        let mut ui = Ui::default();
        ui.touch(Some((100, 800)));
        ui.touch(Some((100, 300)));
        assert!(ui.touch(None).is_none());
        assert_eq!(ui.page, Page::Home);
    }

    #[test]
    fn sliding_onto_a_button_does_not_press_it() {
        let mut ui = Ui::default();
        ui.touch(Some((100, 300)));
        ui.touch(Some((100, 800)));
        assert!(ui.touch(None).is_none());
        assert_eq!(ui.page, Page::Home);
    }

    #[test]
    fn restart_and_power_off_need_a_second_step() {
        let mut ui = Ui::default();
        tap_button(&mut ui, Press::OpenPower);
        assert!(tap_button(&mut ui, Press::AskRestart).is_none());
        assert_eq!(ui.page, Page::ConfirmRestart);
        tap_button(&mut ui, Press::OpenPower);
        assert_eq!(ui.page, Page::Power);
        assert!(tap_button(&mut ui, Press::AskPowerOff).is_none());
        assert!(matches!(tap_button(&mut ui, Press::PowerOff), Some(Request::PowerOff)));

        let mut ui = Ui::default();
        tap_button(&mut ui, Press::OpenPower);
        tap_button(&mut ui, Press::AskRestart);
        assert!(matches!(tap_button(&mut ui, Press::Restart), Some(Request::Restart)));
    }

    #[test]
    fn forgetting_needs_a_second_step() {
        let mut ui = Ui { page: Page::Networks, ..Ui::default() };
        assert!(tap_button(&mut ui, Press::AskForget).is_none());
        assert_eq!(ui.page, Page::ConfirmForget);
        tap_button(&mut ui, Press::OpenNetworksList);
        assert_eq!(ui.page, Page::Networks);
        tap_button(&mut ui, Press::AskForget);
        assert!(matches!(tap_button(&mut ui, Press::Forget), Some(Request::Forget)));
        assert_eq!(ui.page, Page::Networks);
    }

    #[test]
    fn the_confirm_button_is_not_under_the_first_one() {
        // A double tap on POWER OFF or RESTART must not confirm it by accident.
        let mut ui = Ui::default();
        tap_button(&mut ui, Press::OpenPower);
        let first = ui.buttons().into_iter().find(|b| b.press == Press::AskRestart).unwrap().area;
        tap_button(&mut ui, Press::AskRestart);
        let confirm = ui.buttons().into_iter().find(|b| b.press == Press::Restart).unwrap().area;
        assert!(first.y + first.height <= confirm.y);
    }

    #[test]
    fn joining_sends_the_typed_passphrase_then_forgets_it() {
        let mut ui = Ui::default();
        tap_button(&mut ui, Press::OpenNetworks);
        ui.set_networks(vec![network("net")]);
        tap_button(&mut ui, Press::Pick(0));
        type_text(&mut ui, "Pass 123");
        match tap_button(&mut ui, Press::Join) {
            Some(Request::Join(credentials)) => {
                assert_eq!(credentials.ssid, "6e6574");
                assert_eq!(credentials.psk, "\"Pass 123\"");
            }
            _ => panic!("expected a join request"),
        }
        assert!(ui.typed.is_empty());
        assert_eq!(ui.page, Page::Networks);
    }

    #[test]
    fn a_short_passphrase_stays_on_the_keyboard() {
        let mut ui = Ui { page: Page::Networks, ..Ui::default() };
        ui.set_networks(vec![network("net")]);
        tap_button(&mut ui, Press::Pick(0));
        type_text(&mut ui, "short");
        assert!(tap_button(&mut ui, Press::Join).is_none());
        assert_eq!(ui.page, Page::Keyboard);
        assert_eq!(ui.typed, "short");
        assert!(!ui.message.is_empty());
    }

    #[test]
    fn the_space_between_buttons_presses_nothing() {
        let mut ui = Ui { page: Page::Networks, ..Ui::default() };
        ui.set_networks((0..NETWORK_ROWS).map(|index| network(&format!("n{index}"))).collect());
        let between = NETWORK_ROW_TOP + NETWORK_ROW_HEIGHT + 2;
        assert_eq!(ui.press_at((240, between as i32)), None);
        assert_eq!(ui.press_at((4, (NETWORK_ROW_TOP + 5) as i32)), None);

        let ui = Ui::default();
        for index in 0..4 {
            let left = split(BOTTOM_ROW, 5, index);
            let gap = left.x + left.width + BUTTON_GAP / 2;
            assert_eq!(ui.press_at((gap as i32, (BOTTOM_ROW.y + 5) as i32)), None);
        }
        assert_eq!(ui.press_at((4, (BOTTOM_ROW.y + 5) as i32)), None);
    }

    #[test]
    fn changing_page_clears_the_message() {
        let mut ui = Ui::default();
        ui.set_message("Scan failed".to_string());
        tap_button(&mut ui, Press::OpenPower);
        assert!(ui.message.is_empty());
    }

    #[test]
    fn a_request_keeps_its_own_message() {
        let mut ui = Ui::default();
        tap_button(&mut ui, Press::OpenNetworks);
        assert_eq!(ui.message, "Scanning...");
    }

    #[test]
    fn the_passphrase_length_is_limited() {
        let mut ui = Ui { page: Page::Keyboard, ..Ui::default() };
        for _ in 0..70 {
            tap_button(&mut ui, Press::Key('q'));
        }
        assert_eq!(ui.typed.len(), PASSPHRASE_LIMIT);
    }

    #[test]
    fn delete_removes_the_last_character() {
        let mut ui = Ui { page: Page::Keyboard, ..Ui::default() };
        type_text(&mut ui, "ab");
        tap_button(&mut ui, Press::Delete);
        assert_eq!(ui.typed, "a");
        tap_button(&mut ui, Press::Delete);
        tap_button(&mut ui, Press::Delete);
        assert_eq!(ui.typed, "");
    }

    #[test]
    fn the_passphrase_is_hidden_unless_asked_for() {
        let mut ui = Ui { page: Page::Keyboard, ..Ui::default() };
        type_text(&mut ui, "secret");
        assert_eq!(ui.typed_text(), "*****t");
        tap_button(&mut ui, Press::ToggleShow);
        assert_eq!(ui.typed_text(), "secret");
    }

    #[test]
    fn reset_clears_a_typed_passphrase() {
        let mut ui = Ui { page: Page::Keyboard, ..Ui::default() };
        type_text(&mut ui, "secret");
        ui.reset();
        assert!(ui.typed.is_empty());
        assert_eq!(ui.page, Page::Home);
    }

    #[test]
    fn cancel_clears_a_typed_passphrase() {
        let mut ui = Ui { page: Page::Keyboard, ..Ui::default() };
        type_text(&mut ui, "secret");
        tap_button(&mut ui, Press::CancelKeyboard);
        assert!(ui.typed.is_empty());
        assert_eq!(ui.page, Page::Networks);
    }

    #[test]
    fn a_new_scan_while_choosing_cannot_pick_a_missing_network() {
        let mut ui = Ui { page: Page::Networks, ..Ui::default() };
        ui.set_networks(vec![network("a"), network("b")]);
        assert!(ui.press_at((100, (NETWORK_ROW_TOP + NETWORK_ROW_STEP + 5) as i32)).is_some());
        ui.act(Press::Pick(5));
        assert_eq!(ui.page, Page::Networks);
    }

    #[test]
    fn buttons_stay_on_the_screen_and_do_not_overlap() {
        let mut ui = Ui::default();
        ui.set_networks((0..12).map(|index| network(&format!("network {index}"))).collect());
        ui.set_sensors(rows(9), rows(12));
        let keyboards = [Layer::Lower, Layer::Upper, Layer::Symbols, Layer::MoreSymbols];
        let mut pages = vec![
            (Page::Home, Layer::Lower),
            (Page::Power, Layer::Lower),
            (Page::ConfirmRestart, Layer::Lower),
            (Page::ConfirmPowerOff, Layer::Lower),
            (Page::ConfirmForget, Layer::Lower),
            (Page::Networks, Layer::Lower),
            (Page::Bluetooth, Layer::Lower),
            (Page::AddSensor, Layer::Lower),
            (Page::ConfirmRemoveSensor, Layer::Lower),
        ];
        pages.extend(keyboards.map(|layer| (Page::Keyboard, layer)));
        pages.extend(keyboards.map(|layer| (Page::Console, layer)));
        pages.push((Page::Camera, Layer::Lower));
        for (page, layer) in pages {
            ui.page = page;
            ui.layer = layer;
            let buttons = ui.buttons();
            for (index, button) in buttons.iter().enumerate() {
                let area = button.area;
                assert!(area.x + area.width <= WIDTH && area.y + area.height <= 854, "{page:?} {:?}", button.press);
                for other in &buttons[index + 1..] {
                    assert!(!area.overlaps(other.area), "{page:?}: {:?} {:?}", button.press, other.press);
                }
            }
        }
    }

    #[test]
    fn every_fixed_label_fits_its_button() {
        let mut ui = Ui::default();
        let keyboards = [Layer::Lower, Layer::Upper, Layer::Symbols, Layer::MoreSymbols];
        let mut pages: Vec<(Page, Layer)> = [
            Page::Home,
            Page::Power,
            Page::ConfirmRestart,
            Page::ConfirmPowerOff,
            Page::ConfirmForget,
            Page::Networks,
            Page::Bluetooth,
            Page::AddSensor,
            Page::ConfirmRemoveSensor,
            Page::Camera,
        ]
        .map(|page| (page, Layer::Lower))
        .to_vec();
        pages.extend(keyboards.map(|layer| (Page::Keyboard, layer)));
        pages.extend(keyboards.map(|layer| (Page::Console, layer)));
        for (page, layer) in pages {
            ui.page = page;
            ui.layer = layer;
            // Rows of networks and sensors show data, so their length varies.
            let fixed = ui.buttons().into_iter().filter(|button| {
                !matches!(button.press, Press::Pick(_) | Press::PickSaved(_) | Press::PickCandidate(_))
            });
            for button in fixed {
                // The last character's blank column is not drawn, so it may reach the edge.
                let width = (button.label.chars().count() as u32 * CHAR_WIDTH - 1) * button.scale;
                assert!(width <= button.area.width, "{page:?}: {:?} is {width} px in {} px", button.label, button.area.width);
            }
        }
    }

    #[test]
    fn rendering_every_page_does_not_panic() {
        use crate::display::tests::rgba8888;
        let mut canvas = Canvas::new(rgba8888());
        let mut ui = Ui::default();
        ui.set_networks(vec![network("a long network name that fills it")]);
        ui.set_message("A message".to_string());
        let status = Snapshot::default();
        for page in [
            Page::Home,
            Page::Power,
            Page::ConfirmRestart,
            Page::ConfirmPowerOff,
            Page::ConfirmForget,
            Page::Networks,
            Page::Keyboard,
            Page::Bluetooth,
            Page::AddSensor,
            Page::ConfirmRemoveSensor,
            Page::Camera,
            Page::Console,
        ] {
            ui.page = page;
            ui.typed = "x".repeat(PASSPHRASE_LIMIT);
            ui.render(&mut canvas, &status);
        }
        // Full Bluetooth pages, with more rows than fit and over long lines.
        let status = Snapshot {
            bluetooth: "scanning, 256 devices heard".to_string(),
            bluetooth_nearby: (0..40).map(|index| format!("aa:bb:cc:dd:ee:{index:02x}  -60 dBm random")).collect(),
            ..Snapshot::default()
        };
        ui.set_sensors(rows(9), rows(12));
        ui.sensor_target = Some("aa:bb:cc:dd:ee:ff".to_string());
        for page in [Page::Bluetooth, Page::AddSensor, Page::ConfirmRemoveSensor] {
            ui.page = page;
            ui.render(&mut canvas, &status);
        }
    }

    fn rows(count: usize) -> Vec<SensorRow> {
        (0..count)
            .map(|index| SensorRow {
                address: format!("aa:bb:cc:dd:ee:{index:02x}"),
                line: format!("aa:bb:cc:dd:ee:{index:02x} -25.9C 100.0% B100% and more"),
                readable: true,
            })
            .collect()
    }

    #[test]
    fn sensors_are_added_from_what_was_heard() {
        let mut ui = Ui { page: Page::Bluetooth, ..Ui::default() };
        ui.set_sensors(vec![], rows(3));
        assert!(tap_button(&mut ui, Press::OpenAddSensor).is_none());
        assert_eq!(ui.page, Page::AddSensor);
        match tap_button(&mut ui, Press::PickCandidate(1)) {
            Some(Request::AddSensor(address)) => assert_eq!(address, "aa:bb:cc:dd:ee:01"),
            _ => panic!("expected an add request"),
        }
        assert_eq!(ui.page, Page::Bluetooth);
        assert_eq!(ui.message, "Adding...");
        // An encrypted sensor is listed, but says why it cannot be added.
        let mut list = rows(2);
        list[0].readable = false;
        ui.page = Page::AddSensor;
        ui.set_sensors(vec![], list);
        assert!(tap_button(&mut ui, Press::PickCandidate(0)).is_none());
        assert_eq!(ui.page, Page::AddSensor);
        assert_eq!(ui.message, "Encrypted, so it cannot be read.");
        // A list that shrank since it was drawn picks nothing.
        ui.page = Page::AddSensor;
        ui.set_sensors(vec![], rows(1));
        assert!(ui.act(Press::PickCandidate(2)).is_none());
        assert_eq!(ui.page, Page::AddSensor);
    }

    #[test]
    fn removing_a_sensor_needs_a_second_step() {
        let mut ui = Ui { page: Page::Bluetooth, ..Ui::default() };
        ui.set_sensors(rows(2), vec![]);
        assert!(tap_button(&mut ui, Press::PickSaved(1)).is_none());
        assert_eq!(ui.page, Page::ConfirmRemoveSensor);
        assert!(tap_button(&mut ui, Press::OpenBluetooth).is_none());
        assert_eq!(ui.page, Page::Bluetooth);
        tap_button(&mut ui, Press::PickSaved(0));
        match tap_button(&mut ui, Press::RemoveSensor) {
            Some(Request::RemoveSensor(address)) => assert_eq!(address, "aa:bb:cc:dd:ee:00"),
            _ => panic!("expected a remove request"),
        }
        assert_eq!(ui.page, Page::Bluetooth);
        assert_eq!(ui.sensor_target, None);
    }

    #[test]
    fn the_remove_button_is_not_under_a_sensor_row() {
        let mut ui = Ui { page: Page::Bluetooth, ..Ui::default() };
        ui.set_sensors(rows(SENSOR_ROWS), vec![]);
        let rows: Vec<Rect> = ui.buttons().iter().filter(|button| matches!(button.press, Press::PickSaved(_))).map(|button| button.area).collect();
        assert_eq!(rows.len(), SENSOR_ROWS);
        assert!(rows.iter().all(|row| !row.overlaps(REMOVE_ACTION)), "a double tap on a row must not confirm");
    }

    #[test]
    fn camera_page_takes_photos_and_forgets_them() {
        use crate::display::tests::rgba8888;
        let mut ui = Ui::default();
        assert!(tap_button(&mut ui, Press::OpenCamera).is_none());
        assert_eq!(ui.page, Page::Camera);
        assert!(matches!(tap_button(&mut ui, Press::Photo("rear")), Some(Request::Photo("rear"))));
        assert_eq!(ui.message, "Taking a photo...");
        assert!(matches!(tap_button(&mut ui, Press::Photo("front")), Some(Request::Photo("front"))));
        ui.set_photo(&photo::Rgb { width: 972, height: 1296, data: vec![128; 972 * 1296 * 3] });
        let shown = ui.photo.as_ref().map(|image| (image.width, image.height));
        assert_eq!(shown, Some((480, 640)));
        let mut canvas = Canvas::new(rgba8888());
        ui.render(&mut canvas, &Snapshot::default());
        ui.reset();
        assert!(ui.photo.is_none());
        assert_eq!(ui.page, Page::Home);
    }

    #[test]
    fn camera_page_shows_live_video_until_stopped_or_left() {
        use crate::display::tests::rgba8888;
        let frame = fit_photo(&photo::Rgb { width: 486, height: 648, data: vec![90; 486 * 648 * 3] });
        let mut ui = Ui { page: Page::Camera, ..Ui::default() };
        assert!(matches!(tap_button(&mut ui, Press::ToggleLive), Some(Request::Live("rear"))));
        assert_eq!(ui.live, Some("rear"));
        let mut canvas = Canvas::new(rgba8888());
        ui.render(&mut canvas, &Snapshot::default());
        ui.show_live(frame.clone());
        assert_eq!(ui.photo.as_ref().map(|image| (image.width, image.height)), Some((480, 640)));
        ui.render(&mut canvas, &Snapshot::default());

        // REAR and FRONT switch cameras, and the same camera again does nothing.
        assert!(matches!(tap_button(&mut ui, Press::Photo("front")), Some(Request::Live("front"))));
        assert!(ui.photo.is_none(), "a frame from the other camera must not stay");
        assert!(tap_button(&mut ui, Press::Photo("front")).is_none());
        assert!(matches!(tap_button(&mut ui, Press::ToggleLive), Some(Request::StopLive)));
        assert_eq!(ui.live, None);
        // A frame sent before the stop arrives after it and is not shown.
        ui.show_live(frame.clone());
        assert!(ui.photo.is_none());
        // Stopped, REAR takes a photo again.
        assert!(matches!(tap_button(&mut ui, Press::Photo("rear")), Some(Request::Photo("rear"))));

        // Leaving the page stops it.
        tap_button(&mut ui, Press::ToggleLive);
        assert!(matches!(tap_button(&mut ui, Press::Home), Some(Request::StopLive)));
        assert_eq!((ui.page, ui.live), (Page::Home, None));

        // Stopping on its own leaves the last frame and says why.
        ui.page = Page::Camera;
        tap_button(&mut ui, Press::ToggleLive);
        ui.show_live(frame.clone());
        ui.live_ended("Live video ended after 3 min".to_string());
        assert_eq!(ui.live, None);
        assert!(ui.photo.is_some());
        assert!(tap_button(&mut ui, Press::Home).is_none());

        tap_button(&mut ui, Press::OpenCamera);
        tap_button(&mut ui, Press::ToggleLive);
        ui.reset();
        assert_eq!(ui.live, None);
    }

    #[test]
    fn bluetooth_page_turns_the_scan_on_and_off() {
        let mut ui = Ui::default();
        assert!(tap_button(&mut ui, Press::OpenBluetooth).is_none());
        assert_eq!(ui.page, Page::Bluetooth);
        assert!(matches!(tap_button(&mut ui, Press::BluetoothOn), Some(Request::Bluetooth(true))));
        assert_eq!(ui.message, "Turning on...");
        assert!(matches!(tap_button(&mut ui, Press::BluetoothOff), Some(Request::Bluetooth(false))));
        assert_eq!(ui.page, Page::Bluetooth);
        assert!(tap_button(&mut ui, Press::Home).is_none());
        assert_eq!(ui.page, Page::Home);
    }

    #[test]
    fn console_opens_the_shell_and_sends_keys() {
        let mut ui = Ui::default();
        match tap_button(&mut ui, Press::OpenConsole) {
            Some(Request::TerminalSize { columns, rows }) => {
                assert_eq!(columns, TERMINAL_COLUMNS);
                assert_eq!(rows, terminal_rows(true));
            }
            _ => panic!("expected the shell to start"),
        }
        assert_eq!(ui.page, Page::Console);
        let sent = |request: Option<Request>| match request {
            Some(Request::TerminalInput(bytes)) => bytes,
            _ => panic!("expected input for the shell"),
        };
        assert_eq!(sent(tap_button(&mut ui, Press::Key('l'))), b"l");
        assert_eq!(sent(tap_button(&mut ui, Press::Delete)), [0x7F]);
        assert_eq!(sent(tap_button(&mut ui, Press::Enter)), b"\r");
        assert_eq!(sent(tap_button(&mut ui, Press::Tab)), b"\t");
        assert_eq!(sent(tap_button(&mut ui, Press::Escape)), [0x1B]);
        assert_eq!(sent(tap_button(&mut ui, Press::Arrow(b'A'))), b"\x1b[A");
        // Ctrl applies to the next key only.
        assert!(tap_button(&mut ui, Press::Control).is_none());
        assert!(ui.control);
        assert_eq!(sent(tap_button(&mut ui, Press::Key('c'))), [3]);
        assert!(!ui.control);
        assert_eq!(sent(tap_button(&mut ui, Press::Key('c'))), b"c");
        // The Wi-Fi passphrase is not touched.
        assert!(ui.typed.is_empty());
        // Hiding the keyboard gives more lines, and the shell is told.
        match tap_button(&mut ui, Press::ToggleConsoleKeys) {
            Some(Request::TerminalSize { rows, .. }) => assert_eq!(rows, terminal_rows(false)),
            _ => panic!("expected a new size"),
        }
        assert!(terminal_rows(false) > terminal_rows(true) + 10);
    }

    #[test]
    fn dragging_the_console_scrolls_back() {
        let mut ui = Ui { page: Page::Console, ..Ui::default() };
        let rows = ui.terminal.rows();
        ui.terminal_output("line\r\n".repeat(100).as_bytes());
        let total = ui.terminal.lines().len();
        // Pull the text down by five lines.
        let top = (TERMINAL_TOP + 20) as i32;
        assert!(ui.touch(Some((200, top))).is_none());
        assert!(ui.touch(Some((200, top + 5 * TERMINAL_LINE as i32))).is_none());
        assert_eq!(ui.console_scroll, 5);
        // A drag presses nothing when it ends.
        assert!(ui.touch(None).is_none());
        // It stops at the first line.
        ui.touch(Some((200, top)));
        ui.touch(Some((200, top + 400)));
        ui.touch(Some((200, top + 4000)));
        ui.touch(None);
        assert_eq!(ui.console_scroll, total - rows);
        // New output keeps a scrolled view on the same lines.
        let before = ui.console_scroll;
        ui.terminal_output(b"more\r\n");
        assert_eq!(ui.console_scroll, before + 1);
        // Typing shows the end again.
        tap_button(&mut ui, Press::Key('x'));
        assert_eq!(ui.console_scroll, 0);
        // A drag that starts on a key types nothing and scrolls nothing.
        let key = ui.buttons().into_iter().find(|button| button.press == Press::Key('q')).unwrap().area;
        let (x, y) = ((key.x + 5) as i32, (key.y + 5) as i32);
        ui.touch(Some((x, y)));
        ui.touch(Some((x, y - 300)));
        assert!(ui.touch(None).is_none());
        assert_eq!(ui.console_scroll, 0);
    }

    #[test]
    fn console_renders_scrolled_with_and_without_keys() {
        use crate::display::tests::rgba8888;
        let mut canvas = Canvas::new(rgba8888());
        let mut ui = Ui { page: Page::Console, ..Ui::default() };
        ui.terminal_output("x".repeat(5000).as_bytes());
        ui.terminal_note("[the shell ended]");
        ui.console_scroll = 7;
        ui.render(&mut canvas, &Snapshot::default());
        tap_button(&mut ui, Press::ToggleConsoleKeys);
        ui.render(&mut canvas, &Snapshot::default());
        // Without keys only the header buttons are left, and they fit.
        let buttons = ui.buttons();
        assert_eq!(buttons.len(), 2);
        for button in &buttons {
            let width = (button.label.chars().count() as u32 * CHAR_WIDTH - 1) * button.scale;
            assert!(width <= button.area.width, "{:?}", button.label);
        }
    }
}
