use std::cmp::Ordering;
use std::sync::Arc;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use rikka_ui::{Fetch, Label, fetch_blocking, tr};

const FETCHED: &str = "desk.zone";
const MINUS: char = '\u{2212}';

#[derive(Clone, Debug)]
pub(super) enum Resolved {
    Local,
    Away { city: String, zone: Arc<TimeZone> },
    Pending,
    Unknown { city: String },
}

impl Resolved {
    #[must_use]
    pub(crate) fn resolve(name: Option<&str>) -> Resolved {
        let Some(name) = name else {
            return Resolved::Local;
        };
        let owned = name.to_owned();
        match fetch_blocking((FETCHED, owned.clone()), move || TimeZone::get(&owned)) {
            Fetch::Ready(zone) => Resolved::Away {
                city: city(name),
                zone,
            },
            Fetch::Loading => Resolved::Pending,
            Fetch::Failed(_) => Resolved::Unknown { city: city(name) },
        }
    }

    #[must_use]
    pub(crate) fn zone(&self) -> Option<Arc<TimeZone>> {
        match self {
            Resolved::Away { zone, .. } => Some(Arc::clone(zone)),
            Resolved::Local | Resolved::Pending | Resolved::Unknown { .. } => None,
        }
    }

    #[must_use]
    pub(super) fn time_zone(&self) -> TimeZone {
        self.zone()
            .map_or_else(TimeZone::system, |zone| (*zone).clone())
    }

    #[must_use]
    pub(crate) fn city(&self) -> Option<&str> {
        match self {
            Resolved::Away { city, .. } | Resolved::Unknown { city } => Some(city),
            Resolved::Local | Resolved::Pending => None,
        }
    }

    #[must_use]
    pub(super) fn relative(&self, at: Timestamp) -> Option<Label> {
        match self {
            Resolved::Away { zone, .. } => Some(relative(at, zone)),
            Resolved::Unknown { .. } => Some(tr!("shell.desk.widget.clock.unknown-zone")),
            Resolved::Local | Resolved::Pending => None,
        }
    }
}

#[must_use]
pub(crate) fn city(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).replace('_', " ")
}

fn relative(at: Timestamp, zone: &TimeZone) -> Label {
    let here = at.to_zoned(TimeZone::system());
    let there = at.to_zoned(zone.clone());
    let shift = there.offset().seconds() - here.offset().seconds();
    if shift == 0 {
        return tr!("shell.desk.widget.clock.same-time");
    }
    let day = match there.date().cmp(&here.date()) {
        Ordering::Less => tr!("shell.desk.widget.clock.yesterday"),
        Ordering::Equal => tr!("shell.desk.widget.clock.today"),
        Ordering::Greater => tr!("shell.desk.widget.clock.tomorrow"),
    }
    .to_string();
    let shift = shifted(shift).to_string();
    tr!(
        "shell.desk.widget.clock.relative",
        day = day.as_str(),
        shift = shift.as_str(),
    )
}

pub(crate) struct Offset {
    pub(crate) sign: char,
    pub(crate) hours: i32,
    pub(crate) minutes: i32,
}

#[must_use]
pub(crate) fn offset(seconds: i32) -> Offset {
    Offset {
        sign: match seconds < 0 {
            true => MINUS,
            false => '+',
        },
        hours: seconds.abs() / 3600,
        minutes: seconds.abs() % 3600 / 60,
    }
}

fn shifted(seconds: i32) -> Label {
    let Offset {
        sign,
        hours,
        minutes,
    } = offset(seconds);
    let sign = sign.to_string();
    match minutes {
        0 => tr!(
            "shell.desk.widget.clock.hours",
            sign = sign.as_str(),
            hours = hours,
        ),
        _ => tr!(
            "shell.desk.widget.clock.hours-minutes",
            sign = sign.as_str(),
            hours = hours,
            minutes = minutes,
        ),
    }
}
