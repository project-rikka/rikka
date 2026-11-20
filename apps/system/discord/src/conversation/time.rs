use discord_core::TimeStyle;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use rikka_app::prelude::*;

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;
const MONTH: i64 = 30 * DAY;
const YEAR: i64 = 365 * DAY;

pub(crate) fn zoned(millis: i64) -> Option<Zoned> {
    let stamp = Timestamp::from_millisecond(millis).ok()?;
    Some(stamp.to_zoned(TimeZone::system()))
}

pub(crate) fn clock(at: &Zoned) -> String {
    at.strftime("%H:%M").to_string()
}

pub(crate) fn date(at: &Zoned) -> String {
    tr!(
        "discord.conversation.date",
        year = i64::from(at.year()),
        month = i64::from(at.month()),
        day = i64::from(at.day())
    )
    .to_string()
}

pub(crate) fn sent_at(millis: i64) -> String {
    let Some(at) = zoned(millis) else {
        return String::new();
    };
    let today = Zoned::now().date();
    let time = clock(&at);
    if at.date() == today {
        return tr!("discord.conversation.today-at", time = time.as_str()).to_string();
    }
    if today
        .yesterday()
        .is_ok_and(|yesterday| yesterday == at.date())
    {
        return tr!("discord.conversation.yesterday-at", time = time.as_str()).to_string();
    }
    format!("{} {time}", date(&at))
}

pub(crate) fn short_clock(millis: i64) -> String {
    zoned(millis).map(|at| clock(&at)).unwrap_or_default()
}

pub(crate) fn day_of(millis: i64) -> Option<jiff::civil::Date> {
    zoned(millis).map(|at| at.date())
}

pub(crate) fn day_label(millis: i64) -> String {
    zoned(millis).map(|at| date(&at)).unwrap_or_default()
}

pub(crate) fn styled(unix: i64, style: TimeStyle) -> String {
    let Some(at) = zoned(unix.saturating_mul(1000)) else {
        return String::new();
    };
    match style {
        TimeStyle::ShortTime => clock(&at),
        TimeStyle::LongTime => at.strftime("%H:%M:%S").to_string(),
        TimeStyle::ShortDate | TimeStyle::LongDate => date(&at),
        TimeStyle::ShortDateTime => format!("{} {}", date(&at), clock(&at)),
        TimeStyle::LongDateTime => format!("{} {}", date(&at), at.strftime("%H:%M:%S")),
        TimeStyle::Relative => relative(unix, Timestamp::now().as_second()),
    }
}

pub(crate) fn relative(unix: i64, now: i64) -> String {
    let apart = unix - now;
    let span = apart.abs();
    let ahead = apart > 0;
    let (count, unit) = match span {
        _ if span < MINUTE => (span, Unit::Seconds),
        _ if span < HOUR => (span / MINUTE, Unit::Minutes),
        _ if span < DAY => (span / HOUR, Unit::Hours),
        _ if span < MONTH => (span / DAY, Unit::Days),
        _ if span < YEAR => (span / MONTH, Unit::Months),
        _ => (span / YEAR, Unit::Years),
    };
    unit.caption(count, ahead).to_string()
}

#[derive(Clone, Copy)]
enum Unit {
    Seconds,
    Minutes,
    Hours,
    Days,
    Months,
    Years,
}

impl Unit {
    fn caption(self, count: i64, ahead: bool) -> Label {
        match (self, ahead) {
            (Unit::Seconds, false) => tr!("discord.conversation.ago.seconds", count = count),
            (Unit::Minutes, false) => tr!("discord.conversation.ago.minutes", count = count),
            (Unit::Hours, false) => tr!("discord.conversation.ago.hours", count = count),
            (Unit::Days, false) => tr!("discord.conversation.ago.days", count = count),
            (Unit::Months, false) => tr!("discord.conversation.ago.months", count = count),
            (Unit::Years, false) => tr!("discord.conversation.ago.years", count = count),
            (Unit::Seconds, true) => tr!("discord.conversation.in.seconds", count = count),
            (Unit::Minutes, true) => tr!("discord.conversation.in.minutes", count = count),
            (Unit::Hours, true) => tr!("discord.conversation.in.hours", count = count),
            (Unit::Days, true) => tr!("discord.conversation.in.days", count = count),
            (Unit::Months, true) => tr!("discord.conversation.in.months", count = count),
            (Unit::Years, true) => tr!("discord.conversation.in.years", count = count),
        }
    }
}
