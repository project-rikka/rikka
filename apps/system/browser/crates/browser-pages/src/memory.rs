use std::time::Duration;

use browser_store::{TabId, Timestamp};

use crate::hosts::{covers, host_key};

const SOME_PRESSED: f64 = 40.0;
const FULL_PRESSED: f64 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabActivity {
    pub tab: TabId,
    pub live: bool,
    pub seen: bool,
    pub last_active: Timestamp,
    pub playing: bool,
    pub asking: bool,
    pub exempt: bool,
}

impl TabActivity {
    #[must_use]
    pub fn is_discardable(&self, now: Timestamp, idle: Duration) -> bool {
        self.live
            && !self.seen
            && !self.asking
            && !self.exempt
            && !self.playing
            && self.last_active <= now.saturating_sub(idle)
    }
}

#[must_use]
pub fn discardable_tabs(activities: &[TabActivity], now: Timestamp, idle: Duration) -> Vec<TabId> {
    let mut chosen: Vec<&TabActivity> = activities
        .iter()
        .filter(|activity| activity.is_discardable(now, idle))
        .collect();
    chosen.sort_by_key(|activity| activity.last_active);
    chosen.into_iter().map(|activity| activity.tab).collect()
}

#[must_use]
pub fn is_exempt(host: Option<&str>, never: &[String]) -> bool {
    host.is_some_and(|host| never.iter().any(|site| covers(&host_key(site), host)))
}

#[must_use]
pub fn is_pressed(pressure: &str) -> bool {
    pressure.lines().any(|line| {
        let mut words = line.split_whitespace();
        let threshold = match words.next() {
            Some("some") => SOME_PRESSED,
            Some("full") => FULL_PRESSED,
            _ => return false,
        };
        words
            .find_map(|word| word.strip_prefix("avg10="))
            .and_then(|value| value.parse::<f64>().ok())
            .is_some_and(|average| average >= threshold)
    })
}
