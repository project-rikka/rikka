use std::collections::VecDeque;
use std::fmt::{self, Display};
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(60);
const LIMIT: usize = 5;
const FIRST_DELAY: Duration = Duration::from_millis(500);
const MAX_DOUBLINGS: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restart {
    After { delay: Duration, crashes: usize },
    GiveUp,
}

#[derive(Debug, Default)]
pub struct Restarts {
    crashes: VecDeque<Instant>,
}

impl Restarts {
    pub fn crashed(&mut self, at: Instant, guarding: bool) -> Restart {
        self.crashes
            .retain(|crash| at.saturating_duration_since(*crash) < WINDOW);
        self.crashes.push_back(at);
        let crashes = self.crashes.len();
        if crashes >= LIMIT && !guarding {
            return Restart::GiveUp;
        }
        let doublings = u32::try_from(crashes - 1).map_or(MAX_DOUBLINGS, |n| n.min(MAX_DOUBLINGS));
        Restart::After {
            delay: FIRST_DELAY * 2u32.pow(doublings),
            crashes,
        }
    }

    pub fn forget(&mut self) {
        self.crashes.clear();
    }
}

impl Display for Restart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Restart::After { delay, crashes } => write!(
                f,
                "in {delay:?} (crash {crashes} within {}s)",
                WINDOW.as_secs()
            ),
            Restart::GiveUp => write!(
                f,
                "given up after {LIMIT} crashes within {}s; start it with `rikka desktop start`",
                WINDOW.as_secs()
            ),
        }
    }
}
