//! Input event decoding, power key timing, and touches turned into mouse events.
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

/// How long a finger must stay still for the right button, on the desktop.
pub const HOLD_FOR_RIGHT: Duration = Duration::from_millis(1500);
/// How far a finger may wander, in pixels on each axis, and still count as still.
/// A finger held down drifted about 6 pixels on this screen.
const STILL_DISTANCE: i32 = 20;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerButton {
    Left,
    Right,
}

/// What a mouse would do, made from touches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointerEvent {
    Move(i32, i32),
    Press(PointerButton),
    Release(PointerButton),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum GestureState {
    Up,
    /// Down, and not yet a drag or a hold.
    Waiting { start: (i32, i32), since: Duration },
    Dragging,
    /// The right button was clicked; the rest of this touch does nothing.
    Held,
}

/// Turns one finger into a mouse for the desktop. A tap clicks the left button,
/// a finger that moves drags with it, and a finger held still clicks the right one.
pub struct Gesture {
    state: GestureState,
}

impl Default for Gesture {
    fn default() -> Self {
        Self { state: GestureState::Up }
    }
}

impl Gesture {
    /// Takes the finger's position, or `None` when it lifts. `now` is any clock
    /// that never goes backwards.
    pub fn touch(&mut self, position: Option<(i32, i32)>, now: Duration) -> Vec<PointerEvent> {
        use PointerButton::Left;
        use PointerEvent::{Move, Press, Release};
        let mut events = self.tick(now);
        match (self.state, position) {
            (GestureState::Up, Some((x, y))) => {
                self.state = GestureState::Waiting { start: (x, y), since: now };
                events.push(Move(x, y));
            }
            (GestureState::Waiting { start, .. }, Some((x, y))) => {
                if (x - start.0).abs() > STILL_DISTANCE || (y - start.1).abs() > STILL_DISTANCE {
                    // The drag starts where the finger came down.
                    self.state = GestureState::Dragging;
                    events.extend([Move(start.0, start.1), Press(Left), Move(x, y)]);
                }
            }
            (GestureState::Waiting { .. }, None) => {
                self.state = GestureState::Up;
                events.extend([Press(Left), Release(Left)]);
            }
            (GestureState::Dragging, Some((x, y))) => events.push(Move(x, y)),
            (GestureState::Dragging, None) => {
                self.state = GestureState::Up;
                events.push(Release(Left));
            }
            (GestureState::Held, None) => self.state = GestureState::Up,
            (GestureState::Held, Some(_)) | (GestureState::Up, None) => {}
        }
        events
    }

    /// Clicks the right button once a finger has been still long enough.
    pub fn tick(&mut self, now: Duration) -> Vec<PointerEvent> {
        match self.state {
            GestureState::Waiting { start, since } if now.saturating_sub(since) >= HOLD_FOR_RIGHT => {
                self.state = GestureState::Held;
                vec![
                    PointerEvent::Move(start.0, start.1),
                    PointerEvent::Press(PointerButton::Right),
                    PointerEvent::Release(PointerButton::Right),
                ]
            }
            _ => Vec::new(),
        }
    }

    /// How long until `tick` may click, or `None` if no finger is waiting.
    pub fn wait_limit(&self, now: Duration) -> Option<Duration> {
        match self.state {
            GestureState::Waiting { since, .. } => Some(HOLD_FOR_RIGHT.saturating_sub(now.saturating_sub(since))),
            _ => None,
        }
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

    const fn millis(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    use PointerButton::{Left, Right};
    use PointerEvent::{Move, Press, Release};

    #[test]
    fn tap_clicks_left_where_the_finger_was() {
        let mut gesture = Gesture::default();
        assert_eq!(gesture.touch(Some((100, 200)), millis(0)), vec![Move(100, 200)]);
        // A small wobble is still a tap.
        assert_eq!(gesture.touch(Some((106, 195)), millis(50)), vec![]);
        assert_eq!(gesture.touch(None, millis(120)), vec![Press(Left), Release(Left)]);
        assert_eq!(gesture.touch(None, millis(130)), vec![]);
    }

    #[test]
    fn moving_finger_drags_from_where_it_came_down() {
        let mut gesture = Gesture::default();
        gesture.touch(Some((100, 200)), millis(0));
        assert_eq!(
            gesture.touch(Some((140, 200)), millis(40)),
            vec![Move(100, 200), Press(Left), Move(140, 200)]
        );
        assert_eq!(gesture.touch(Some((150, 210)), millis(60)), vec![Move(150, 210)]);
        // A drag never turns into a right click, however long it lasts.
        assert_eq!(gesture.tick(millis(5000)), vec![]);
        assert_eq!(gesture.touch(None, millis(5000)), vec![Release(Left)]);
    }

    #[test]
    fn still_finger_clicks_right_once() {
        let mut gesture = Gesture::default();
        gesture.touch(Some((300, 400)), millis(0));
        assert_eq!(gesture.wait_limit(millis(200)), Some(millis(1300)));
        assert_eq!(gesture.tick(millis(1499)), vec![]);
        assert_eq!(gesture.tick(millis(1500)), vec![Move(300, 400), Press(Right), Release(Right)]);
        assert_eq!(gesture.wait_limit(millis(1600)), None);
        // Moving or lifting afterwards does nothing more.
        assert_eq!(gesture.touch(Some((360, 400)), millis(1650)), vec![]);
        assert_eq!(gesture.touch(None, millis(1680)), vec![]);
        // The next touch starts afresh.
        assert_eq!(gesture.touch(Some((10, 20)), millis(3000)), vec![Move(10, 20)]);
    }

    #[test]
    fn late_report_still_clicks_right_before_lifting() {
        // Without a tick in time, the next report finds the hold first.
        let mut gesture = Gesture::default();
        gesture.touch(Some((300, 400)), millis(0));
        assert_eq!(gesture.touch(None, millis(2200)), vec![Move(300, 400), Press(Right), Release(Right)]);
        assert_eq!(gesture.wait_limit(millis(2200)), None);
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
