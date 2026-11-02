//! Power key handling: event decoding and press timing.
//!
//! The logic takes the time as an argument and does no I/O, so it runs on the
//! host in unit tests.

use std::time::Duration;

/// Size of `struct input_event` on 32-bit ARM: a 32-bit `timeval`, then the type,
/// the code and the value.
pub const EVENT_LEN: usize = 16;

const EV_SYN: u16 = 0;
const EV_KEY: u16 = 1;
const EV_ABS: u16 = 3;
const SYN_REPORT: u16 = 0;
const BTN_TOUCH: u16 = 330;
const ABS_MT_POSITION_X: u16 = 0x35;
const ABS_MT_POSITION_Y: u16 = 0x36;
const KEY_POWER: u16 = 116;
const KEY_RELEASED: i32 = 0;
const KEY_PRESSED: i32 = 1;

const LONG_PRESS: Duration = Duration::from_secs(3);

#[derive(Debug, PartialEq)]
pub enum Action {
    ToggleScreen,
    PowerOff,
}

#[derive(Debug, PartialEq)]
pub enum PowerKeyEvent {
    Pressed,
    Released,
}

/// Splits a raw input event into its type, code and value.
fn decode(raw: &[u8; EVENT_LEN]) -> (u16, u16, i32) {
    (
        u16::from_ne_bytes([raw[8], raw[9]]),
        u16::from_ne_bytes([raw[10], raw[11]]),
        i32::from_ne_bytes([raw[12], raw[13], raw[14], raw[15]]),
    )
}

/// Tracks the touch screen. With several fingers, it follows the last one reported.
#[derive(Default)]
pub struct Touch {
    x: i32,
    y: i32,
    down: bool,
}

impl Touch {
    /// Applies a raw event. At the end of each report it returns the touched
    /// position, or `Some(None)` when nothing touches the screen.
    pub fn event(&mut self, raw: &[u8; EVENT_LEN]) -> Option<Option<(i32, i32)>> {
        match decode(raw) {
            (EV_ABS, ABS_MT_POSITION_X, value) => self.x = value,
            (EV_ABS, ABS_MT_POSITION_Y, value) => self.y = value,
            (EV_KEY, BTN_TOUCH, value) => self.down = value != 0,
            (EV_SYN, SYN_REPORT, _) => return Some(self.down.then_some((self.x, self.y))),
            _ => {}
        }
        None
    }
}

/// Decodes a raw input event. Other keys, event types and auto-repeat are `None`.
pub fn parse_event(raw: &[u8; EVENT_LEN]) -> Option<PowerKeyEvent> {
    let (kind, code, value) = decode(raw);
    if kind != EV_KEY || code != KEY_POWER {
        return None;
    }
    match value {
        KEY_PRESSED => Some(PowerKeyEvent::Pressed),
        KEY_RELEASED => Some(PowerKeyEvent::Released),
        _ => None,
    }
}

/// Tracks one key press. `now` is any clock that never goes backwards.
#[derive(Default)]
pub struct PowerKey {
    pressed_at: Option<Duration>,
}

impl PowerKey {
    /// Applies a key event. A short press acts when the key is released.
    pub fn event(&mut self, event: PowerKeyEvent, now: Duration) -> Option<Action> {
        match event {
            PowerKeyEvent::Pressed => {
                self.pressed_at = Some(now);
                None
            }
            PowerKeyEvent::Released => self.pressed_at.take().map(|_| Action::ToggleScreen),
        }
    }

    /// Reports a long press as soon as the key has been held long enough.
    pub fn tick(&mut self, now: Duration) -> Option<Action> {
        let pressed_at = self.pressed_at?;
        if now.saturating_sub(pressed_at) >= LONG_PRESS {
            self.pressed_at = None;
            Some(Action::PowerOff)
        } else {
            None
        }
    }

    /// How long to wait for the next event before calling `tick`. `None` means
    /// that no key is held, so waiting forever is fine.
    pub fn wait_limit(&self, now: Duration) -> Option<Duration> {
        self.pressed_at
            .map(|pressed_at| LONG_PRESS.saturating_sub(now.saturating_sub(pressed_at)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn seconds(value: u64) -> Duration {
        Duration::from_secs(value)
    }

    fn raw(kind: u16, code: u16, value: i32) -> [u8; EVENT_LEN] {
        let mut event = [0xAA; EVENT_LEN];
        event[8..10].copy_from_slice(&kind.to_ne_bytes());
        event[10..12].copy_from_slice(&code.to_ne_bytes());
        event[12..16].copy_from_slice(&value.to_ne_bytes());
        event
    }

    #[test]
    fn touch_reports_position_then_release() {
        let mut touch = Touch::default();
        assert_eq!(touch.event(&raw(1, 330, 1)), None);
        assert_eq!(touch.event(&raw(3, 0x35, 120)), None);
        assert_eq!(touch.event(&raw(3, 0x36, 640)), None);
        assert_eq!(touch.event(&raw(0, 0, 0)), Some(Some((120, 640))));

        assert_eq!(touch.event(&raw(3, 0x35, 130)), None);
        assert_eq!(touch.event(&raw(0, 0, 0)), Some(Some((130, 640))));

        assert_eq!(touch.event(&raw(1, 330, 0)), None);
        assert_eq!(touch.event(&raw(0, 0, 0)), Some(None));
    }

    #[test]
    fn decodes_only_power_key_changes() {
        assert_eq!(parse_event(&raw(1, 116, 1)), Some(PowerKeyEvent::Pressed));
        assert_eq!(parse_event(&raw(1, 116, 0)), Some(PowerKeyEvent::Released));
        // Auto-repeat, a volume key and a sync event.
        assert_eq!(parse_event(&raw(1, 116, 2)), None);
        assert_eq!(parse_event(&raw(1, 115, 1)), None);
        assert_eq!(parse_event(&raw(0, 0, 0)), None);
    }

    #[test]
    fn short_press_toggles_the_screen_on_release() {
        let mut key = PowerKey::default();
        assert_eq!(key.event(PowerKeyEvent::Pressed, seconds(10)), None);
        assert_eq!(key.tick(seconds(12)), None);
        assert_eq!(
            key.event(PowerKeyEvent::Released, seconds(12)),
            Some(Action::ToggleScreen)
        );
    }

    #[test]
    fn long_press_powers_off_while_held_and_only_once() {
        let mut key = PowerKey::default();
        key.event(PowerKeyEvent::Pressed, seconds(10));
        assert_eq!(key.tick(seconds(13)), Some(Action::PowerOff));
        assert_eq!(key.tick(seconds(14)), None);
        assert_eq!(key.event(PowerKeyEvent::Released, seconds(15)), None);
    }

    #[test]
    fn release_without_press_does_nothing() {
        let mut key = PowerKey::default();
        assert_eq!(key.event(PowerKeyEvent::Released, seconds(1)), None);
    }

    #[test]
    fn wait_limit_counts_down_only_while_held() {
        let mut key = PowerKey::default();
        assert_eq!(key.wait_limit(seconds(10)), None);
        key.event(PowerKeyEvent::Pressed, seconds(10));
        assert_eq!(key.wait_limit(seconds(11)), Some(seconds(2)));
        assert_eq!(key.wait_limit(seconds(20)), Some(seconds(0)));
    }
}
