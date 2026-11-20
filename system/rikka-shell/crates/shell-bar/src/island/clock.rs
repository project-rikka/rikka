use std::time::Duration;

use jiff::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use rikka_ui::{Label, tr};
use shell_feeds::{NameLength, weekday};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Clock {
    date: Date,
    hour: i8,
    minute: i8,
}

impl Clock {
    #[must_use]
    pub(crate) fn new(at: Timestamp) -> Clock {
        let now = at.to_zoned(TimeZone::system());
        Clock {
            date: now.date(),
            hour: now.hour(),
            minute: now.minute(),
        }
    }

    #[must_use]
    pub(crate) fn time(self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    #[must_use]
    pub(crate) fn date(self) -> Label {
        let weekday = weekday(self.date.weekday(), NameLength::Short).to_string();
        tr!(
            "shell.island.date",
            weekday = weekday.as_str(),
            day = self.date.day(),
            month = self.date.month(),
        )
    }

    #[must_use]
    pub(crate) fn long_date(self) -> Label {
        let weekday = weekday(self.date.weekday(), NameLength::Long).to_string();
        tr!(
            "shell.island.long-date",
            weekday = weekday.as_str(),
            day = self.date.day(),
            month = self.date.month(),
        )
    }
}

#[must_use]
pub(crate) fn span(length: Duration) -> String {
    let seconds = length.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    match hours {
        0 => format!("{minutes}:{seconds:02}"),
        _ => format!("{hours}:{minutes:02}:{seconds:02}"),
    }
}
