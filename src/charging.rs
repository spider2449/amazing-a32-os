//! Watches the charger and the battery. The stock battery services do not run, so
//! nothing else notices when charging stops or the battery runs out. The decisions
//! have no I/O, so they run on the host in unit tests.
//!
//! The kernel sometimes reads a low charger voltage as USB is plugged in and then
//! reports `Not charging` until USB is plugged in again. Software has no known way
//! to clear that, so this only makes it visible.

use std::time::Duration;

/// One reading of the power supply files.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// USB or mains power is attached.
    pub charger: bool,
    /// The kernel's status, such as `Charging`, `Full`, `Not charging`, `Discharging`.
    pub status: String,
    /// The kernel's estimate, in percent. It depends on how the phone started.
    pub capacity: Option<u32>,
    pub battery_millivolts: Option<u32>,
    pub charger_millivolts: Option<u32>,
}

impl Reading {
    /// The battery driver reads 0 mV, 50% and no charger for its first moments,
    /// about 10 seconds after the kernel starts. Such a reading means nothing.
    pub fn is_ready(&self) -> bool {
        self.battery_millivolts.is_some_and(|millivolts| millivolts > 0)
    }

    fn is_charging(&self) -> bool {
        matches!(self.status.as_str(), "Charging" | "Full")
    }

    pub fn describe(&self) -> String {
        let show = |value: Option<u32>, unit: &str| value.map_or("?".to_string(), |value| format!("{value}{unit}"));
        format!(
            "{} {}, battery {}, charger {} at {}",
            show(self.capacity, "%"),
            self.status,
            show(self.battery_millivolts, " mV"),
            if self.charger { "attached" } else { "removed" },
            show(self.charger_millivolts, " mV"),
        )
    }
}

/// Something worth a log line, or an action.
#[derive(Debug, PartialEq)]
pub enum Event {
    ChargerAttached,
    ChargerRemoved,
    /// Attached, but not charging for the whole `STALL_LIMIT`.
    Stalled,
    /// Charging again after a stall.
    Resumed,
    /// Running on a nearly empty battery. The phone should power off cleanly.
    Empty,
}

// Charging can pause briefly, for example while the kernel checks the charger.
const STALL_LIMIT: Duration = Duration::from_secs(5 * 60);
// Lower than a working battery reads under this phone's light load. Either one is
// enough, because the percentage is only an estimate.
const EMPTY_PERCENT: u32 = 3;
const EMPTY_MILLIVOLTS: u32 = 3400;
// Readings in a row, so that one bad reading cannot turn the phone off.
const EMPTY_READINGS: u32 = 2;

#[derive(Default)]
pub struct Monitor {
    charger: Option<bool>,
    not_charging_since: Option<Duration>,
    stalled: bool,
    empty_readings: u32,
}

impl Monitor {
    pub fn is_stalled(&self) -> bool {
        self.stalled
    }

    /// Takes a reading made at `now` and returns what changed.
    pub fn update(&mut self, now: Duration, reading: &Reading) -> Vec<Event> {
        let mut events = Vec::new();
        if !reading.is_ready() {
            return events;
        }
        match self.charger.replace(reading.charger) {
            Some(false) if reading.charger => events.push(Event::ChargerAttached),
            Some(true) if !reading.charger => events.push(Event::ChargerRemoved),
            _ => {}
        }

        if reading.charger && !reading.is_charging() {
            let since = *self.not_charging_since.get_or_insert(now);
            if !self.stalled && now.saturating_sub(since) >= STALL_LIMIT {
                self.stalled = true;
                events.push(Event::Stalled);
            }
        } else {
            self.not_charging_since = None;
            if self.stalled {
                self.stalled = false;
                // Unplugging ends a stall too, but that is not charging again.
                if reading.charger {
                    events.push(Event::Resumed);
                }
            }
        }

        let low = reading.capacity.is_some_and(|percent| percent <= EMPTY_PERCENT)
            || reading.battery_millivolts.is_some_and(|millivolts| millivolts < EMPTY_MILLIVOLTS);
        self.empty_readings = if !reading.charger && low { self.empty_readings + 1 } else { 0 };
        if self.empty_readings == EMPTY_READINGS {
            events.push(Event::Empty);
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn s(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    fn reading(charger: bool, status: &str, capacity: u32, millivolts: u32) -> Reading {
        Reading {
            charger,
            status: status.to_string(),
            capacity: Some(capacity),
            battery_millivolts: Some(millivolts),
            charger_millivolts: Some(if charger { 4700 } else { 0 }),
        }
    }

    #[test]
    fn reports_the_charger_coming_and_going_but_not_at_start() {
        let mut monitor = Monitor::default();
        assert_eq!(monitor.update(s(0), &reading(true, "Charging", 50, 4000)), vec![]);
        assert_eq!(monitor.update(s(60), &reading(false, "Discharging", 50, 3900)), vec![Event::ChargerRemoved]);
        assert_eq!(monitor.update(s(120), &reading(true, "Charging", 50, 4000)), vec![Event::ChargerAttached]);
    }

    #[test]
    fn reports_a_stall_once_after_the_limit_then_the_recovery() {
        let mut monitor = Monitor::default();
        let stuck = reading(true, "Not charging", 50, 3900);
        assert_eq!(monitor.update(s(0), &stuck), vec![]);
        assert_eq!(monitor.update(s(299), &stuck), vec![]);
        assert_eq!(monitor.update(s(300), &stuck), vec![Event::Stalled]);
        assert!(monitor.is_stalled());
        assert_eq!(monitor.update(s(360), &stuck), vec![]);
        assert_eq!(monitor.update(s(420), &reading(true, "Charging", 50, 4000)), vec![Event::Resumed]);
        assert!(!monitor.is_stalled());
    }

    #[test]
    fn a_short_pause_and_a_full_battery_are_not_stalls() {
        let mut monitor = Monitor::default();
        assert_eq!(monitor.update(s(0), &reading(true, "Not charging", 50, 3900)), vec![]);
        assert_eq!(monitor.update(s(200), &reading(true, "Charging", 50, 4000)), vec![]);
        assert_eq!(monitor.update(s(400), &reading(true, "Not charging", 50, 3900)), vec![]);
        assert_eq!(monitor.update(s(1000), &reading(true, "Full", 100, 4350)), vec![]);
        assert!(!monitor.is_stalled());
    }

    #[test]
    fn unplugging_ends_a_stall_without_calling_it_resumed() {
        let mut monitor = Monitor::default();
        let stuck = reading(true, "Not charging", 50, 3900);
        monitor.update(s(0), &stuck);
        monitor.update(s(300), &stuck);
        assert_eq!(monitor.update(s(360), &reading(false, "Discharging", 50, 3850)), vec![Event::ChargerRemoved]);
        assert!(!monitor.is_stalled());
    }

    #[test]
    fn empty_needs_two_low_readings_on_battery() {
        let mut monitor = Monitor::default();
        assert_eq!(monitor.update(s(0), &reading(false, "Discharging", 3, 3600)), vec![]);
        assert_eq!(monitor.update(s(60), &reading(false, "Discharging", 4, 3600)), vec![]);
        assert_eq!(monitor.update(s(120), &reading(false, "Discharging", 10, 3390)), vec![]);
        assert_eq!(monitor.update(s(180), &reading(false, "Discharging", 10, 3380)), vec![Event::Empty]);
        // Reported once, not on every later reading.
        assert_eq!(monitor.update(s(240), &reading(false, "Discharging", 2, 3370)), vec![]);
    }

    #[test]
    fn a_low_battery_on_the_charger_is_not_empty() {
        let mut monitor = Monitor::default();
        for minute in 0..5 {
            assert_eq!(monitor.update(s(minute * 60), &reading(true, "Charging", 1, 3300)), vec![]);
        }
    }

    #[test]
    fn readings_before_the_driver_is_ready_are_ignored() {
        let mut monitor = Monitor::default();
        // As read on the phone 10 seconds after the kernel started.
        let early = reading(false, "Not charging", 50, 0);
        for minute in 0..5 {
            assert_eq!(monitor.update(s(minute * 60), &early), vec![]);
        }
        // The first real reading is not a charger change either.
        assert_eq!(monitor.update(s(300), &reading(true, "Charging", 96, 4357)), vec![]);
    }

    #[test]
    fn missing_values_are_neither_low_nor_a_crash() {
        let mut monitor = Monitor::default();
        let unknown = Reading {
            charger: false,
            status: String::new(),
            capacity: None,
            battery_millivolts: None,
            charger_millivolts: None,
        };
        assert_eq!(monitor.update(s(0), &unknown), vec![]);
        assert_eq!(monitor.update(s(60), &unknown), vec![]);
        assert_eq!(unknown.describe(), "? , battery ?, charger removed at ?");
    }
}
