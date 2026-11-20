use jiff::civil::{Date, Weekday};
use rikka_ui::{Label, tr};

use crate::days::day_key;
use crate::{NameLength, weekday};

#[must_use]
pub fn long_date(date: Date) -> Label {
    let weekday = weekday(date.weekday(), NameLength::Long).to_string();
    tr!(
        "shell.desk.widget.date.long",
        weekday = weekday.as_str(),
        day = date.day(),
        month = date.month(),
    )
}

#[must_use]
pub fn day_month(date: Date) -> Label {
    tr!(
        "shell.desk.widget.date.day-month",
        day = date.day(),
        month = date.month(),
    )
}

#[must_use]
pub fn month(date: Date) -> Label {
    tr!("shell.desk.widget.date.month", month = date.month())
}

#[must_use]
pub fn month_of_year(date: Date) -> Label {
    tr!(
        "shell.desk.widget.date.month-year",
        month = date.month(),
        year = date.year(),
    )
}

#[must_use]
pub fn initial(day: Weekday) -> Label {
    tr!(&format!("shell.desk.widget.date.initial.{}", day_key(day)))
}

#[must_use]
pub fn week(date: Date) -> Label {
    tr!(
        "shell.desk.widget.clock.week",
        week = date.iso_week_date().week(),
    )
}

#[must_use]
pub fn time(hour: i8, minute: i8) -> String {
    format!("{hour:02}:{minute:02}")
}
