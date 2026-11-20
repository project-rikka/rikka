use jiff::Zoned;
use jiff::civil::Weekday;
use rikka_app::prelude::*;

pub(crate) fn long_date(now: &Zoned) -> Label {
    tr!(
        "settings.date-time.date",
        weekday = weekday(now.weekday()).to_string().as_str(),
        month = month(now.month()).to_string().as_str(),
        day = now.day(),
        year = now.year(),
    )
}

pub(crate) fn clock(now: &Zoned) -> String {
    format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second())
}

fn weekday(day: Weekday) -> Label {
    Label::translated(match day {
        Weekday::Monday => "settings.date-time.weekday.mon",
        Weekday::Tuesday => "settings.date-time.weekday.tue",
        Weekday::Wednesday => "settings.date-time.weekday.wed",
        Weekday::Thursday => "settings.date-time.weekday.thu",
        Weekday::Friday => "settings.date-time.weekday.fri",
        Weekday::Saturday => "settings.date-time.weekday.sat",
        Weekday::Sunday => "settings.date-time.weekday.sun",
    })
}

fn month(month: i8) -> Label {
    const MONTHS: [&str; 12] = [
        "settings.date-time.month.jan",
        "settings.date-time.month.feb",
        "settings.date-time.month.mar",
        "settings.date-time.month.apr",
        "settings.date-time.month.may",
        "settings.date-time.month.jun",
        "settings.date-time.month.jul",
        "settings.date-time.month.aug",
        "settings.date-time.month.sep",
        "settings.date-time.month.oct",
        "settings.date-time.month.nov",
        "settings.date-time.month.dec",
    ];
    let index = usize::try_from(month - 1)
        .unwrap_or(0)
        .min(MONTHS.len() - 1);
    Label::translated(MONTHS[index])
}
