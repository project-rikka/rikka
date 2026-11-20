use std::time::{Duration, SystemTime, UNIX_EPOCH};

use music_core::{Album, Fidelity, Release, Service};
use rikka_app::prelude::*;
use rikka_format::{format_duration, format_timestamp};

const HERTZ_PER_KILOHERTZ: u32 = 1000;
const HERTZ_PER_MEGAHERTZ: u32 = 1_000_000;
const TENTHS: u32 = 100;
const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;

pub(crate) fn songs(count: usize) -> Label {
    tr!("music.count.songs", count = &count.to_string())
}

pub(crate) fn albums(count: usize) -> Label {
    tr!("music.count.albums", count = &count.to_string())
}

pub(crate) fn service(service: &Service) -> Label {
    match service.is_local() {
        true => tr!("music.local.name"),
        false => Label::from(service.name),
    }
}

pub(crate) fn length(duration: Duration) -> String {
    match duration.is_zero() {
        true => String::new(),
        false => format_duration(duration),
    }
}

pub(crate) fn span(duration: Duration) -> String {
    let minutes = (duration.as_secs() + MINUTE / 2) / MINUTE;
    match duration.as_secs() < HOUR {
        true => tr!("music.span.minutes", minutes = &minutes.max(1).to_string()).to_string(),
        false => tr!(
            "music.span.hours",
            hours = &(minutes / 60).to_string(),
            minutes = &(minutes % 60).to_string()
        )
        .to_string(),
    }
}

pub(crate) fn day(at: SystemTime) -> String {
    let seconds = at
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let stamped = format_timestamp(i64::try_from(seconds).unwrap_or_default());
    stamped
        .split_once(' ')
        .map_or(stamped.clone(), |(day, _)| day.to_owned())
}

pub(crate) fn hertz(rate: u32) -> String {
    match rate >= HERTZ_PER_MEGAHERTZ {
        true => tr!("music.unit.mhz", rate = &megahertz(rate)).to_string(),
        false => tr!("music.unit.khz", rate = &kilohertz(rate)).to_string(),
    }
}

pub(crate) fn dsd(rate: u32) -> String {
    format!("DSD{}", Fidelity::dsd_multiple(rate).unwrap_or_default())
}

pub(crate) fn release(album: &Album) -> String {
    let kind = match album.release {
        Release::Album => tr!("music.release.album"),
        Release::Single => tr!("music.release.single"),
        Release::Ep => tr!("music.release.ep"),
        Release::Compilation => tr!("music.release.compilation"),
    };
    match album.year {
        Some(year) => format!("{kind} · {year}"),
        None => kind.to_string(),
    }
}

pub(crate) fn fidelity(fidelity: Fidelity) -> String {
    let codec = fidelity.codec.label();
    let detail: Option<Label> = match (fidelity.dsd, fidelity.resolution, fidelity.kbps) {
        (Some(dsd), _, _) => Some(tr!(
            "music.fidelity.dsd",
            multiple = &Fidelity::dsd_multiple(dsd).unwrap_or_default().to_string(),
            rate = &megahertz(dsd)
        )),
        (None, Some(resolution), _) => Some(tr!(
            "music.fidelity.format",
            bits = &resolution.bits.to_string(),
            rate = &kilohertz(resolution.rate)
        )),
        (None, None, Some(kbps)) => Some(tr!("music.fidelity.bitrate", kbps = &kbps.to_string())),
        (None, None, None) => None,
    };
    match detail {
        Some(detail) if fidelity.dsd.is_some() => detail.to_string(),
        Some(detail) => format!("{codec} · {detail}"),
        None => codec.to_owned(),
    }
}

fn megahertz(hertz: u32) -> String {
    let tenths = (hertz + HERTZ_PER_MEGAHERTZ / 20) / (HERTZ_PER_MEGAHERTZ / 10);
    format!("{}.{}", tenths / 10, tenths % 10)
}

fn kilohertz(hertz: u32) -> String {
    let whole = hertz / HERTZ_PER_KILOHERTZ;
    let tenths = (hertz % HERTZ_PER_KILOHERTZ + TENTHS / 2) / TENTHS;
    match tenths {
        0 => whole.to_string(),
        10 => (whole + 1).to_string(),
        tenths => format!("{whole}.{tenths}"),
    }
}
