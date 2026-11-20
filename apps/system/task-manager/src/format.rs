use std::time::Duration;

use rikka_format::{format_bytes, group_digits};
use rikka_i18n::tr;

pub(crate) const UNKNOWN: &str = "--";

pub(crate) fn rate(per_second: f32) -> String {
    tr!("units.per-second", value = &format_bytes(per_second as u64)).to_string()
}

pub(crate) fn bits(bytes_per_second: f32) -> String {
    let megabits = bytes_per_second * 8.0 / 1_000_000.0;
    let value = decimal(megabits, 1);
    tr!("units.megabits", value = &value).to_string()
}

pub(crate) fn percent(fraction: f32, places: usize) -> String {
    format!("{}%", decimal(fraction * 100.0, places))
}

pub(crate) fn ghz(value: f32) -> String {
    let value = decimal(value, 2);
    tr!("units.ghz", value = &value).to_string()
}

pub(crate) fn uptime(elapsed: Duration) -> String {
    let seconds = elapsed.as_secs();
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    format!(
        "{days}:{:02}:{:02}:{:02}",
        rest / 3600,
        (rest % 3600) / 60,
        rest % 60
    )
}

fn decimal(value: f32, places: usize) -> String {
    group_digits(&format!("{value:.places$}"))
}
