//! When to slow the CPU or stop the phone because of heat. The stock service that
//! enables the kernel's thermal zones does not run, so this is the only protection.
//! The decisions have no I/O, so they run on the host in unit tests.

const MILLIDEGREES: i64 = 1000;
// The hysteresis stops the speed changing back and forth around one temperature.
const CAP_RELEASE_MARGIN: i64 = 10 * MILLIDEGREES;
const CPU_POWER_OFF: i64 = 95 * MILLIDEGREES;
const BATTERY_POWER_OFF: i64 = 55 * MILLIDEGREES;
// Disconnected sensors read values such as -127 C.
const VALID_RANGE: std::ops::RangeInclusive<i64> = (-40 * MILLIDEGREES)..=(150 * MILLIDEGREES);

#[derive(Debug, PartialEq)]
pub enum Action {
    None,
    Cap,
    Release,
    PowerOff,
}

pub struct Policy {
    cap_at: i64,
    capped: bool,
}

impl Policy {
    pub fn new(cap_at_degrees: i64) -> Self {
        Self { cap_at: cap_at_degrees * MILLIDEGREES, capped: false }
    }

    pub fn is_capped(&self) -> bool {
        self.capped
    }

    /// Takes temperatures in millidegrees. A missing or invalid one is ignored.
    pub fn decide(&mut self, cpu: Option<i64>, battery: Option<i64>) -> Action {
        let valid = |value: &i64| VALID_RANGE.contains(value);
        let (cpu, battery) = (cpu.filter(valid), battery.filter(valid));
        if cpu.is_some_and(|value| value >= CPU_POWER_OFF)
            || battery.is_some_and(|value| value >= BATTERY_POWER_OFF)
        {
            return Action::PowerOff;
        }
        let Some(cpu) = cpu else {
            return Action::None;
        };
        if !self.capped && cpu >= self.cap_at {
            self.capped = true;
            Action::Cap
        } else if self.capped && cpu <= self.cap_at - CAP_RELEASE_MARGIN {
            self.capped = false;
            Action::Release
        } else {
            Action::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn c(degrees: i64) -> Option<i64> {
        Some(degrees * MILLIDEGREES)
    }

    #[test]
    fn caps_then_releases_with_hysteresis() {
        let mut policy = Policy::new(80);
        assert_eq!(policy.decide(c(79), c(30)), Action::None);
        assert_eq!(policy.decide(c(80), c(30)), Action::Cap);
        assert!(policy.is_capped());
        assert_eq!(policy.decide(c(85), c(30)), Action::None);
        assert_eq!(policy.decide(c(71), c(30)), Action::None);
        assert_eq!(policy.decide(c(70), c(30)), Action::Release);
        assert!(!policy.is_capped());
        assert_eq!(policy.decide(c(70), c(30)), Action::None);
    }

    #[test]
    fn powers_off_at_the_last_resort_limits() {
        let mut policy = Policy::new(80);
        assert_eq!(policy.decide(c(95), c(30)), Action::PowerOff);
        assert_eq!(policy.decide(c(40), c(55)), Action::PowerOff);
        assert_eq!(policy.decide(None, c(55)), Action::PowerOff);
    }

    #[test]
    fn ignores_missing_and_invalid_sensors() {
        let mut policy = Policy::new(80);
        assert_eq!(policy.decide(None, None), Action::None);
        assert_eq!(policy.decide(c(-127), c(30)), Action::None);
        assert_eq!(policy.decide(c(500), c(500)), Action::None);
        assert!(!policy.is_capped());
    }
}
