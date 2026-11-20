use jiff::{Timestamp, Zoned};
use rikka_app::prelude::*;

const WEEK: i32 = 7;

#[must_use]
pub(crate) fn moment(at: Timestamp) -> String {
    let at = zoned(at);
    let now = Zoned::now();

    if at.date() == now.date() {
        return format!("{:02}:{:02}", at.hour(), at.minute());
    }
    if is_within_week(&now, &at) {
        return named("weekday", i64::from(at.weekday().to_monday_one_offset()));
    }
    if at.year() == now.year() {
        return format!("{} {}", at.day(), month(at.month()));
    }
    format!("{} {} {}", at.day(), month(at.month()), at.year())
}

#[must_use]
pub(crate) fn whole(at: Timestamp) -> String {
    let at = zoned(at);
    format!(
        "{} {} {} {:02}:{:02}",
        at.day(),
        month(at.month()),
        at.year(),
        at.hour(),
        at.minute()
    )
}

#[must_use]
pub(crate) fn size(bytes: i64) -> String {
    rikka_format::format_bytes(u64::try_from(bytes).unwrap_or_default())
}

#[must_use]
pub(crate) fn kind(entry: &proton_files::Entry) -> String {
    if entry.folder {
        return tr!("proton.drive.kind-folder").to_string();
    }
    match entry.suffix() {
        Some(suffix) => tr!(
            "proton.drive.kind-extension",
            extension = &suffix.to_uppercase()
        )
        .to_string(),
        None => tr!("proton.drive.kind-generic").to_string(),
    }
}

#[must_use]
pub(crate) fn count(number: usize) -> String {
    rikka_format::format_count(u64::try_from(number).unwrap_or(u64::MAX))
}

#[must_use]
pub(crate) fn initial(text: &str) -> String {
    text.chars()
        .find(|letter| letter.is_alphanumeric())
        .map(|letter| letter.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_owned())
}

fn month(number: i8) -> String {
    named("month", i64::from(number).clamp(1, 12))
}

fn named(kind: &str, at: i64) -> String {
    tr!(&format!("proton.{kind}.{at}")).as_str().to_owned()
}

fn is_within_week(now: &Zoned, at: &Zoned) -> bool {
    now.date()
        .since(at.date())
        .is_ok_and(|span| (0..WEEK).contains(&span.get_days()))
}

fn zoned(at: Timestamp) -> Zoned {
    at.to_zoned(jiff::tz::TimeZone::system())
}
