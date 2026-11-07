//! Choices made on the panel's settings page. They are kept on /data as
//! `name=value` lines with real values, not positions in a list, so that the file
//! can be read and changed by hand. No I/O here, so it runs in host tests.

use std::time::Duration;

/// How long the screen stays on after the last use, in seconds. `None` keeps it on,
/// which costs idle CPU time, because the panel is drawn every second.
const SCREEN_TIMEOUTS: [Option<u64>; 8] = [Some(15), Some(30), Some(60), Some(120), Some(300), Some(600), Some(1800), None];
/// Backlight levels, out of the kernel's 255. 64 was the fixed level before, and
/// the lowest is still readable indoors.
const BACKLIGHT_LEVELS: [u32; 6] = [16, 32, 64, 128, 192, 255];
/// How long a finger must stay still on the desktop for the right button.
const HOLD_TIMES_MS: [u64; 5] = [800, 1000, 1500, 2000, 2500];

/// One line of the settings page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    ScreenOff,
    Brightness,
    TouchWakes,
    RightClickHold,
}

impl Setting {
    pub const ALL: [Setting; 4] = [Setting::ScreenOff, Setting::Brightness, Setting::TouchWakes, Setting::RightClickHold];

    /// The name on the page.
    pub fn label(self) -> &'static str {
        match self {
            Setting::ScreenOff => "SCREEN OFF AFTER",
            Setting::Brightness => "BRIGHTNESS",
            Setting::TouchWakes => "TOUCH WAKES THE SCREEN",
            Setting::RightClickHold => "DESKTOP RIGHT CLICK HOLD",
        }
    }

    /// The two buttons' labels, lower then higher.
    pub fn buttons(self) -> (&'static str, &'static str) {
        match self {
            Setting::TouchWakes => ("OFF", "ON"),
            _ => ("-", "+"),
        }
    }

    /// The name in the file.
    fn key(self) -> &'static str {
        match self {
            Setting::ScreenOff => "screen-off",
            Setting::Brightness => "brightness",
            Setting::TouchWakes => "touch-wakes",
            Setting::RightClickHold => "right-click-hold",
        }
    }
}

/// The chosen values, as positions in the lists above, so that every value is one
/// of the offered ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    screen_off: usize,
    brightness: usize,
    touch_wakes: bool,
    hold: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl Settings {
    /// What the system did before these could be changed.
    pub const DEFAULT: Settings = Settings { screen_off: 2, brightness: 2, touch_wakes: true, hold: 2 };

    pub fn screen_timeout(&self) -> Option<Duration> {
        SCREEN_TIMEOUTS[self.screen_off].map(Duration::from_secs)
    }

    /// The backlight level, as text for the kernel's `brightness` file.
    pub fn backlight(&self) -> String {
        BACKLIGHT_LEVELS[self.brightness].to_string()
    }

    pub fn touch_wakes(&self) -> bool {
        self.touch_wakes
    }

    pub fn right_click_hold(&self) -> Duration {
        Duration::from_millis(HOLD_TIMES_MS[self.hold])
    }

    /// The value as the page shows it.
    pub fn value_text(&self, setting: Setting) -> String {
        match setting {
            Setting::ScreenOff => match SCREEN_TIMEOUTS[self.screen_off] {
                None => "never".to_string(),
                Some(seconds) if seconds < 60 => format!("{seconds} s"),
                Some(seconds) => format!("{} min", seconds / 60),
            },
            Setting::Brightness => format!("{}%", (BACKLIGHT_LEVELS[self.brightness] * 100 + 127) / 255),
            Setting::TouchWakes => if self.touch_wakes { "on" } else { "off" }.to_string(),
            Setting::RightClickHold => {
                let millis = HOLD_TIMES_MS[self.hold];
                format!("{}.{} s", millis / 1000, millis % 1000 / 100)
            }
        }
    }

    /// Moves a setting one step up or down. Returns false at the end of its range,
    /// where nothing changes.
    pub fn step(&mut self, setting: Setting, up: bool) -> bool {
        let before = *self;
        let next = |index: usize, count: usize| if up { (index + 1).min(count - 1) } else { index.saturating_sub(1) };
        match setting {
            Setting::ScreenOff => self.screen_off = next(self.screen_off, SCREEN_TIMEOUTS.len()),
            Setting::Brightness => self.brightness = next(self.brightness, BACKLIGHT_LEVELS.len()),
            Setting::TouchWakes => self.touch_wakes = up,
            Setting::RightClickHold => self.hold = next(self.hold, HOLD_TIMES_MS.len()),
        }
        *self != before
    }

    pub fn to_text(self) -> String {
        let screen_off = SCREEN_TIMEOUTS[self.screen_off].map_or("never".to_string(), |seconds| seconds.to_string());
        let lines = [
            (Setting::ScreenOff, screen_off),
            (Setting::Brightness, BACKLIGHT_LEVELS[self.brightness].to_string()),
            (Setting::TouchWakes, if self.touch_wakes { "on" } else { "off" }.to_string()),
            (Setting::RightClickHold, HOLD_TIMES_MS[self.hold].to_string()),
        ];
        lines.iter().map(|(setting, value)| format!("{}={value}\n", setting.key())).collect()
    }

    /// Reads the file. A missing line, an unknown name or a value not offered
    /// leaves that setting at its default, so a damaged file cannot stop the panel.
    pub fn from_text(text: &str) -> Settings {
        let mut settings = Settings::DEFAULT;
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else { continue };
            let (key, value) = (key.trim(), value.trim());
            let position = |found: Option<usize>, current: usize| found.unwrap_or(current);
            if key == Setting::ScreenOff.key() {
                let wanted = if value == "never" { Some(None) } else { value.parse().ok().map(Some) };
                let found = wanted.and_then(|wanted| SCREEN_TIMEOUTS.iter().position(|&option| option == wanted));
                settings.screen_off = position(found, settings.screen_off);
            } else if key == Setting::Brightness.key() {
                let found = value.parse().ok().and_then(|level: u32| BACKLIGHT_LEVELS.iter().position(|&option| option == level));
                settings.brightness = position(found, settings.brightness);
            } else if key == Setting::TouchWakes.key() {
                match value {
                    "on" => settings.touch_wakes = true,
                    "off" => settings.touch_wakes = false,
                    _ => {}
                }
            } else if key == Setting::RightClickHold.key() {
                let found = value.parse().ok().and_then(|millis: u64| HOLD_TIMES_MS.iter().position(|&option| option == millis));
                settings.hold = position(found, settings.hold);
            }
        }
        settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_match_the_old_fixed_values() {
        let settings = Settings::default();
        assert_eq!(settings.screen_timeout(), Some(Duration::from_secs(60)));
        assert_eq!(settings.backlight(), "64");
        assert!(settings.touch_wakes());
        assert_eq!(settings.right_click_hold(), Duration::from_millis(1500));
    }

    #[test]
    fn steps_stop_at_the_ends() {
        let mut settings = Settings::default();
        assert!(settings.step(Setting::ScreenOff, false));
        assert!(settings.step(Setting::ScreenOff, false));
        assert_eq!(settings.value_text(Setting::ScreenOff), "15 s");
        assert!(!settings.step(Setting::ScreenOff, false));
        for _ in 0..10 {
            settings.step(Setting::ScreenOff, true);
        }
        assert_eq!(settings.screen_timeout(), None);
        assert_eq!(settings.value_text(Setting::ScreenOff), "never");
        assert!(!settings.step(Setting::ScreenOff, true));

        assert!(settings.step(Setting::TouchWakes, false));
        assert!(!settings.step(Setting::TouchWakes, false));
        assert_eq!(settings.value_text(Setting::TouchWakes), "off");
    }

    #[test]
    fn values_read_as_text() {
        let mut settings = Settings::default();
        assert_eq!(settings.value_text(Setting::ScreenOff), "1 min");
        assert_eq!(settings.value_text(Setting::Brightness), "25%");
        assert_eq!(settings.value_text(Setting::RightClickHold), "1.5 s");
        settings.step(Setting::RightClickHold, false);
        settings.step(Setting::RightClickHold, false);
        assert_eq!(settings.value_text(Setting::RightClickHold), "0.8 s");
        for _ in 0..10 {
            settings.step(Setting::Brightness, true);
        }
        assert_eq!(settings.value_text(Setting::Brightness), "100%");
    }

    #[test]
    fn the_file_keeps_every_value() {
        let mut settings = Settings::default();
        settings.step(Setting::ScreenOff, true);
        settings.step(Setting::Brightness, false);
        settings.step(Setting::TouchWakes, false);
        settings.step(Setting::RightClickHold, true);
        let text = settings.to_text();
        assert_eq!(text, "screen-off=120\nbrightness=32\ntouch-wakes=off\nright-click-hold=2000\n");
        assert_eq!(Settings::from_text(&text), settings);

        for _ in 0..10 {
            settings.step(Setting::ScreenOff, true);
        }
        assert_eq!(Settings::from_text(&settings.to_text()), settings);
    }

    #[test]
    fn bad_lines_keep_the_defaults() {
        assert_eq!(Settings::from_text(""), Settings::default());
        let text = "screen-off=7\nbrightness=bright\ntouch-wakes=maybe\nnope=1\njunk\nright-click-hold=\n";
        assert_eq!(Settings::from_text(text), Settings::default());
        let partly = Settings::from_text(" brightness = 255 \nscreen-off=never\n");
        assert_eq!(partly.backlight(), "255");
        assert_eq!(partly.screen_timeout(), None);
        assert!(partly.touch_wakes());
    }
}
