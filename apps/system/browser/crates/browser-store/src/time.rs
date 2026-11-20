use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::ToSql;
use rusqlite::types::{FromSql, FromSqlResult, ToSqlOutput, ValueRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Timestamp(i64);

impl Timestamp {
    pub const MIN: Self = Self(i64::MIN);
    pub const MAX: Self = Self(i64::MAX);

    #[must_use]
    pub fn now() -> Self {
        Self::from(SystemTime::now())
    }

    #[must_use]
    pub const fn from_unix_millis(millis: i64) -> Self {
        Self(millis)
    }

    #[must_use]
    pub const fn unix_millis(self) -> i64 {
        self.0
    }

    #[must_use]
    pub fn saturating_sub(self, duration: Duration) -> Self {
        Self(self.0.saturating_sub(millis(duration)))
    }

    #[must_use]
    pub fn saturating_add(self, duration: Duration) -> Self {
        Self(self.0.saturating_add(millis(duration)))
    }
}

fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

impl From<SystemTime> for Timestamp {
    fn from(time: SystemTime) -> Self {
        match time.duration_since(UNIX_EPOCH) {
            Ok(after) => Self(millis(after)),
            Err(before) => Self(-millis(before.duration())),
        }
    }
}

impl From<Timestamp> for SystemTime {
    fn from(time: Timestamp) -> Self {
        let offset = Duration::from_millis(time.0.unsigned_abs());
        if time.0 >= 0 {
            UNIX_EPOCH.checked_add(offset).unwrap_or(UNIX_EPOCH)
        } else {
            UNIX_EPOCH.checked_sub(offset).unwrap_or(UNIX_EPOCH)
        }
    }
}

impl ToSql for Timestamp {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.0))
    }
}

impl FromSql for Timestamp {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeRange {
    pub from: Timestamp,
    pub to: Timestamp,
}

impl TimeRange {
    #[must_use]
    pub const fn new(from: Timestamp, to: Timestamp) -> Self {
        Self { from, to }
    }

    #[must_use]
    pub const fn all() -> Self {
        Self::new(Timestamp::MIN, Timestamp::MAX)
    }

    #[must_use]
    pub const fn since(from: Timestamp) -> Self {
        Self::new(from, Timestamp::MAX)
    }

    #[must_use]
    pub const fn before(to: Timestamp) -> Self {
        Self::new(Timestamp::MIN, to)
    }
}
