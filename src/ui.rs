//! The touch panel: pages, buttons and the on-screen keyboard. It has no I/O: it takes
//! touches and status text, and returns requests, so it runs on the host in unit tests.

use crate::display::{Canvas, Rect, Rgb, ACCENT, BACKGROUND, FOREGROUND};
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
}

/// Work the panel cannot do itself, because it would stop the screen responding.
pub enum Request {
    Restart,
    PowerOff,
    Scan,
    Join(Credentials),
    Forget,
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
        }
    }
}

impl Ui {
    pub fn set_networks(&mut self, networks: Vec<Network>) {
        self.networks = networks;
    }

    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }

    /// Back to the home page, without anything typed. Used when the screen turns off,
    /// so that a passphrase does not stay on the panel.
    pub fn reset(&mut self) {
        self.page = Page::Home;
        self.pressed = None;
        self.finger_down = false;
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
                }
                self.position = position;
                None
            }
            None => {
                self.finger_down = false;
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
            Page::Home => vec![
                Button::new(split(BOTTOM_ROW, 2, 0), "WI-FI", Press::OpenNetworks),
                Button::new(split(BOTTOM_ROW, 2, 1), "POWER", Press::OpenPower),
            ],
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
        }
    }

    fn keyboard_buttons(&self) -> Vec<Button> {
        const KEY_WIDTH: u32 = WIDTH / 10;
        let mut buttons = Vec::new();
        let rows = self.layer.rows();
        for (row, keys) in rows.iter().enumerate() {
            let count = keys.chars().count() as u32;
            let y = KEY_TOP + row as u32 * KEY_HEIGHT;
            let mut left = (WIDTH - count * KEY_WIDTH) / 2;
            if row == 2 {
                buttons.push(Button::new(
                    Rect { x: 0, y, width: left.max(KEY_WIDTH), height: KEY_HEIGHT },
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
                    Rect { x: left + index as u32 * KEY_WIDTH, y, width: KEY_WIDTH, height: KEY_HEIGHT },
                    &key.to_string(),
                    Press::Key(key),
                ));
            }
            if row == 2 {
                let x = left + count * KEY_WIDTH;
                buttons.push(Button::new(
                    Rect { x, y, width: WIDTH - x, height: KEY_HEIGHT },
                    "DEL",
                    Press::Delete,
                ));
            }
        }
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
        }
        for button in self.buttons() {
            let color = if self.pressed == Some(button.press) { PRESSED } else { button.color };
            canvas.fill(button.area.inset(2), color);
            canvas.draw_text_centered(button.area, button.scale, &button.label, FOREGROUND);
        }
        if !self.message.is_empty() && self.page != Page::Home {
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
        assert_eq!(ui.press_at((240, (BOTTOM_ROW.y + 5) as i32)), None);
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
        let keyboards = [Layer::Lower, Layer::Upper, Layer::Symbols, Layer::MoreSymbols];
        let mut pages = vec![
            (Page::Home, Layer::Lower),
            (Page::Power, Layer::Lower),
            (Page::ConfirmRestart, Layer::Lower),
            (Page::ConfirmPowerOff, Layer::Lower),
            (Page::ConfirmForget, Layer::Lower),
            (Page::Networks, Layer::Lower),
        ];
        pages.extend(keyboards.map(|layer| (Page::Keyboard, layer)));
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
        ] {
            ui.page = page;
            ui.typed = "x".repeat(PASSPHRASE_LIMIT);
            ui.render(&mut canvas, &status);
        }
    }
}
