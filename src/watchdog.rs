//! When to restart the phone because something stopped working. Restarting is the
//! only repair an unattended phone can make for itself.
//!
//! The kernel kicks the hardware watchdog from its own threads, so it only covers a
//! hung kernel. MediaTek's hang detector, `/dev/RT_Monitor`, was tried on this phone:
//! it only dumps diagnostics when it expires and never restarted, so it is not used.
//! The decisions here have no I/O, so they run on the host in unit tests.

use std::time::Duration;

/// A loop that reports a heartbeat each time round.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Service {
    /// Collects exited children. It runs every second on the main thread.
    Reaper,
    /// Slows the CPU when hot. Without it nothing protects the phone from heat.
    Thermal,
    /// Renews the lease and sets the clock. One round can include a DHCP exchange,
    /// DNS lookups and three time servers, which take up to about a minute.
    Network,
    /// Draws the screen and reacts to touches, at least once a second.
    Panel,
}

impl Service {
    pub const ALL: [Service; 4] = [Service::Reaper, Service::Thermal, Service::Network, Service::Panel];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Service::Reaper => "reaper",
            Service::Thermal => "thermal",
            Service::Network => "network",
            Service::Panel => "panel",
        }
    }

    /// How long the loop may go without a heartbeat. Each limit is many times the
    /// loop's longest normal round, because a needless restart also costs service.
    pub fn limit(self) -> Duration {
        match self {
            Service::Reaper | Service::Thermal | Service::Panel => Duration::from_secs(180),
            Service::Network => Duration::from_secs(600),
        }
    }
}

/// Finds a service whose last heartbeat is older than its limit. `beats` holds the
/// time of each service's last heartbeat, by `Service::index`, or `None` for a
/// service that has not started or has stopped on purpose.
pub fn stalled(beats: &[Option<Duration>; 4], now: Duration) -> Option<(Service, Duration)> {
    Service::ALL.into_iter().find_map(|service| {
        let silent = now.saturating_sub(beats[service.index()]?);
        (silent > service.limit()).then_some((service, silent))
    })
}

/// The first wait for a lost network. Long enough for a router to restart or a
/// short outage to pass, because a restart drops every connection.
pub const NETWORK_LOST_LIMIT: Duration = Duration::from_secs(30 * 60);
const NETWORK_LOST_LONGEST: Duration = Duration::from_secs(24 * 3600);

/// How long a lost network is tolerated, after `restarts` restarts in a row for the
/// same reason. Doubling stops a router that is gone for days from restarting the
/// phone every half hour.
pub fn network_lost_limit(first: Duration, restarts: u32) -> Duration {
    first.saturating_mul(1u32 << restarts.min(16)).min(NETWORK_LOST_LONGEST.max(first))
}

/// Watches whether the network works, and decides when a restart may repair it.
#[derive(Default)]
pub struct NetworkWatch {
    lost_since: Option<Duration>,
}

impl NetworkWatch {
    /// Takes whether a network is saved and whether it works now. Returns true when
    /// it has not worked for `limit`. Without a saved network nothing is expected.
    pub fn update(&mut self, now: Duration, saved: bool, working: bool, limit: Duration) -> bool {
        if !saved || working {
            self.lost_since = None;
            return false;
        }
        let since = *self.lost_since.get_or_insert(now);
        now.saturating_sub(since) >= limit
    }
}

/// Whether a panic should restart normally instead of going to fastboot. Fastboot
/// is where a broken image is repaired, but an unattended phone waits there forever.
/// An image that ran this long booted and started its console, so a later panic is
/// treated as a passing fault. A panic in every session still restarts at most once
/// per `stable`.
pub fn panic_restarts_normally(uptime: Duration, stable: Duration) -> bool {
    uptime >= stable
}

/// Formats seconds since 1970 as UTC, such as `2026-11-06T11:31:02Z`, without a
/// time zone database. Uses the days-to-civil method of Howard Hinnant.
pub fn utc_text(unix: u64) -> String {
    let days = (unix / 86400) as i64;
    let seconds = unix % 86400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    )
}

/// Reads the first line of `/proc/aed/reboot-reason`, such as
/// `WDT status: 0 fiq step: 0  exception type: 0`, which tells whether the hardware
/// watchdog or a kernel exception ended the previous session.
pub fn previous_reset(reboot_reason: &str) -> Option<&str> {
    reboot_reason.lines().map(str::trim).find(|line| line.starts_with("WDT status:"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn s(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn finds_only_a_service_past_its_limit() {
        let mut beats = [None; 4];
        assert_eq!(stalled(&beats, s(10_000)), None);
        beats[Service::Thermal.index()] = Some(s(1000));
        beats[Service::Network.index()] = Some(s(1000));
        assert_eq!(stalled(&beats, s(1180)), None);
        assert_eq!(stalled(&beats, s(1181)), Some((Service::Thermal, s(181))));
        beats[Service::Thermal.index()] = Some(s(1590));
        assert_eq!(stalled(&beats, s(1600)), None);
        assert_eq!(stalled(&beats, s(1601)), Some((Service::Network, s(601))));
    }

    #[test]
    fn a_heartbeat_from_after_now_is_not_a_stall() {
        let beats = [Some(s(50)), None, None, None];
        assert_eq!(stalled(&beats, s(10)), None);
    }

    #[test]
    fn network_limit_doubles_and_stops_growing() {
        let first = s(1800);
        assert_eq!(network_lost_limit(first, 0), s(1800));
        assert_eq!(network_lost_limit(first, 1), s(3600));
        assert_eq!(network_lost_limit(first, 4), s(28_800));
        assert_eq!(network_lost_limit(first, 6), s(86_400));
        assert_eq!(network_lost_limit(first, u32::MAX), s(86_400));
        // A test build's short first limit still grows.
        assert_eq!(network_lost_limit(s(60), 2), s(240));
    }

    #[test]
    fn network_restart_needs_a_saved_network_lost_for_the_whole_limit() {
        let limit = s(1800);
        let mut watch = NetworkWatch::default();
        assert!(!watch.update(s(0), false, false, limit));
        assert!(!watch.update(s(5000), false, false, limit));
        assert!(!watch.update(s(5000), true, false, limit));
        assert!(!watch.update(s(6799), true, false, limit));
        assert!(watch.update(s(6800), true, false, limit));
        // Working again, even once, starts the wait over.
        assert!(!watch.update(s(6805), true, true, limit));
        assert!(!watch.update(s(6810), true, false, limit));
        assert!(!watch.update(s(8609), true, false, limit));
        assert!(watch.update(s(8610), true, false, limit));
        // Forgetting the network ends the wait.
        assert!(!watch.update(s(8615), false, false, limit));
    }

    #[test]
    fn panic_goes_to_fastboot_only_early() {
        assert!(!panic_restarts_normally(s(30), s(600)));
        assert!(!panic_restarts_normally(s(599), s(600)));
        assert!(panic_restarts_normally(s(600), s(600)));
    }

    #[test]
    fn formats_utc() {
        assert_eq!(utc_text(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_text(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc_text(1_793_964_662), "2026-11-06T11:31:02Z");
        assert_eq!(utc_text(4_107_542_399), "2100-02-28T23:59:59Z");
    }

    #[test]
    fn reads_the_previous_reset() {
        let text = "\nWDT status: 2 fiq step: 0  exception type: 1\n[LAST PC] CORE_0 PC = 0x0\n";
        assert_eq!(previous_reset(text), Some("WDT status: 2 fiq step: 0  exception type: 1"));
        assert_eq!(previous_reset("nothing here"), None);
    }
}
