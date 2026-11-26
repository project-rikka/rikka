use rikka_app::prelude::*;

const KILO: f64 = 1_000.0;
const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

pub(crate) fn count(value: u64) -> String {
    let value = value as f64;
    let (scaled, suffix) = if value >= KILO * KILO * KILO {
        (value / (KILO * KILO * KILO), "B")
    } else if value >= KILO * KILO {
        (value / (KILO * KILO), "M")
    } else if value >= KILO {
        (value / KILO, "K")
    } else {
        return format!("{value:.0}");
    };
    match scaled < 10.0 {
        true => format!("{scaled:.1}{suffix}"),
        false => format!("{scaled:.0}{suffix}"),
    }
}

pub(crate) fn bytes(value: u64) -> String {
    let value = value as f64;
    let units = ["B", "KB", "MB", "GB"];
    let mut scaled = value;
    let mut unit = 0;
    while scaled >= 1024.0 && unit < units.len() - 1 {
        scaled /= 1024.0;
        unit += 1;
    }
    match unit {
        0 => format!("{scaled:.0} {}", units[unit]),
        _ => format!("{scaled:.1} {}", units[unit]),
    }
}

pub(crate) fn play_time(seconds: u64) -> Label {
    let seconds = i64::try_from(seconds).unwrap_or(i64::MAX);
    match seconds {
        0 => tr!("launcher.time.never-played"),
        seconds if seconds < HOUR => tr!(
            "launcher.time.minutes",
            count = &(seconds / MINUTE).max(1).to_string()
        ),
        seconds => tr!(
            "launcher.time.hours",
            count = &format!("{:.1}", seconds as f64 / HOUR as f64)
        ),
    }
}

pub(crate) fn ago(unix: i64) -> Label {
    let elapsed = (jiff::Timestamp::now().as_second() - unix).max(0);
    match elapsed {
        elapsed if elapsed < MINUTE => tr!("launcher.time.now"),
        elapsed if elapsed < HOUR => tr!(
            "launcher.time.minutes-ago",
            count = &(elapsed / MINUTE).to_string()
        ),
        elapsed if elapsed < DAY => tr!(
            "launcher.time.hours-ago",
            count = &(elapsed / HOUR).to_string()
        ),
        elapsed => tr!(
            "launcher.time.days-ago",
            count = &(elapsed / DAY).to_string()
        ),
    }
}

pub(crate) fn iso_ago(iso: &str) -> Option<Label> {
    iso.parse::<jiff::Timestamp>()
        .ok()
        .map(|instant| ago(instant.as_second()))
}
