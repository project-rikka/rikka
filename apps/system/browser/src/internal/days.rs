use browser_store::Timestamp;
use jiff::civil::Date;
use jiff::tz::TimeZone;
use rikka_app::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LocalTime {
    pub(crate) date: Date,
    pub(crate) time: String,
}

pub(crate) fn local_time(at: Timestamp) -> Option<LocalTime> {
    let zoned = jiff::Timestamp::from_millisecond(at.unix_millis())
        .ok()?
        .to_zoned(TimeZone::system());
    Some(LocalTime {
        date: zoned.date(),
        time: format!("{:02}:{:02}", zoned.hour(), zoned.minute()),
    })
}

pub(crate) fn today() -> Date {
    jiff::Zoned::now().date()
}

pub(crate) fn day(date: Date, today: Date) -> String {
    if date == today {
        return tr!("browser.day.today").to_string();
    }
    if today.yesterday().is_ok_and(|yesterday| yesterday == date) {
        return tr!("browser.day.yesterday").to_string();
    }
    tr!(
        "browser.day.date",
        weekday = &named("weekday", i64::from(date.weekday().to_monday_one_offset())),
        month = &named("month", i64::from(date.month())),
        day = &date.day().to_string(),
        year = &date.year().to_string()
    )
    .to_string()
}

pub(crate) fn weekday_and_date(date: Date) -> String {
    tr!(
        "browser.home.date",
        weekday = &named("weekday", i64::from(date.weekday().to_monday_one_offset())),
        month = &named("month", i64::from(date.month())),
        day = &date.day().to_string()
    )
    .to_string()
}

fn named(kind: &str, at: i64) -> String {
    tr!(&format!("browser.{kind}.{at}")).to_string()
}
