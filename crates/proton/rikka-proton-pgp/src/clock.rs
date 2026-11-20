use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Clock(Source);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
enum Source {
    #[default]
    System,
    Fixed(u64),
    Offset(i64),
}

impl Clock {
    #[must_use]
    pub fn system() -> Self {
        Clock(Source::System)
    }

    #[must_use]
    pub fn fixed(unix_seconds: u64) -> Self {
        Clock(Source::Fixed(unix_seconds))
    }

    #[must_use]
    pub fn offset(seconds: i64) -> Self {
        Clock(Source::Offset(seconds))
    }

    #[must_use]
    pub fn now(&self) -> u64 {
        match self.0 {
            Source::System => system_seconds(),
            Source::Fixed(seconds) => seconds,
            Source::Offset(offset) => system_seconds().saturating_add_signed(offset),
        }
    }
}

fn system_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}
