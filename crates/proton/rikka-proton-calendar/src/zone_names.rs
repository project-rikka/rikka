use std::cmp::Reverse;
use std::collections::HashMap;
use std::sync::LazyLock;

const WINDOWS_ZONES_TABLE: &str = include_str!("../data/windows-zones.txt");
const STANDARD_TIME: &str = "standard time";

static WINDOWS_ZONES: LazyLock<HashMap<String, &'static str>> = LazyLock::new(|| {
    WINDOWS_ZONES_TABLE
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('\t'))
        .map(|(windows, iana)| (normalize_name(windows), iana))
        .collect()
});

static REGIONAL_ZONES: LazyLock<Vec<(&'static str, String)>> = LazyLock::new(|| {
    let mut names: Vec<(&'static str, String)> = jiff_tzdb::available()
        .filter(|name| name.contains('/') && !name.starts_with("Etc/"))
        .map(|name| (name, name.to_lowercase()))
        .collect();
    names.sort_by_key(|(name, _)| Reverse(name.len()));
    names
});

fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase().replace('.', "")
}

fn strip_utc_offset(tzid: &str) -> &str {
    let trimmed = tzid.trim();
    let upper = trimmed.to_ascii_uppercase();
    if (upper.starts_with("(UTC") || upper.starts_with("(GMT"))
        && let Some(end) = trimmed.find(')')
    {
        return trimmed[end + 1..].trim();
    }
    for marker in [" (UTC", " (GMT"] {
        if let Some(start) = upper.find(marker) {
            return trimmed[..start].trim();
        }
    }
    trimmed
}

#[must_use]
pub fn windows_zone(name: &str) -> Option<&'static str> {
    WINDOWS_ZONES.get(&normalize_name(name)).copied()
}

pub(crate) fn iana_zone_for(tzid: &str) -> Option<String> {
    let stripped = strip_utc_offset(tzid);
    if let Some(zone) = windows_zone(stripped) {
        return Some(zone.to_owned());
    }

    let described = match stripped.find('(') {
        Some(open) => stripped[..open].trim(),
        None => stripped,
    };
    let lowered = normalize_name(described);
    let standard = match lowered.strip_suffix(" time") {
        Some(region) => format!("{region} {STANDARD_TIME}"),
        None => format!("{lowered} {STANDARD_TIME}"),
    };
    if let Some(zone) = WINDOWS_ZONES.get(&standard) {
        return Some((*zone).to_owned());
    }

    for city in stripped
        .split(',')
        .map(str::trim)
        .filter(|city| !city.is_empty())
    {
        let target = city.to_lowercase().replace(' ', "_");
        if let Some((name, _)) = REGIONAL_ZONES
            .iter()
            .find(|(_, lowered)| lowered.rsplit('/').next() == Some(target.as_str()))
        {
            return Some((*name).to_owned());
        }
    }

    let lowered = tzid.to_lowercase();
    REGIONAL_ZONES
        .iter()
        .find(|(_, name)| lowered.contains(name.as_str()))
        .map(|(name, _)| (*name).to_owned())
}
