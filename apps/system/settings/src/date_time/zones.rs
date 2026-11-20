use std::collections::HashMap;
use std::path::PathBuf;

use jiff::Timestamp;
use jiff::tz::TimeZone;

const ROOT_ENV: &str = "TZDIR";

const ROOTS: [&str; 3] = [
    "/etc/zoneinfo",
    "/run/current-system/sw/share/zoneinfo",
    "/usr/share/zoneinfo",
];

const ZONES: &str = "zone.tab";
const COUNTRIES: &str = "iso3166.tab";
const UNIVERSAL: &str = "UTC";

#[derive(Clone, Debug)]
pub(crate) struct Zone {
    pub(crate) name: String,
    pub(crate) city: String,
    pub(crate) country: Option<String>,
    zone: TimeZone,
    search: String,
}

impl Zone {
    pub(crate) fn named(name: &str) -> Option<Zone> {
        Zone::placed(name, None)
    }

    fn placed(name: &str, country: Option<String>) -> Option<Zone> {
        let zone = TimeZone::get(name).ok()?;
        let city = name.rsplit('/').next().unwrap_or(name).replace('_', " ");
        let search = [name, city.as_str(), country.as_deref().unwrap_or_default()]
            .join("\n")
            .to_lowercase();
        Some(Zone {
            name: name.to_owned(),
            city,
            country,
            zone,
            search,
        })
    }

    pub(crate) fn offset(&self, at: Timestamp) -> String {
        let seconds = self.zone.to_offset(at).seconds();
        let sign = if seconds < 0 { '-' } else { '+' };
        let seconds = seconds.unsigned_abs();
        format!("UTC{sign}{:02}:{:02}", seconds / 3600, seconds / 60 % 60)
    }

    pub(crate) fn clock(&self, at: Timestamp) -> String {
        let local = at.to_zoned(self.zone.clone());
        format!("{:02}:{:02}", local.hour(), local.minute())
    }

    pub(crate) fn matches(&self, query: &str) -> bool {
        self.search.contains(query)
    }
}

pub(crate) fn installed() -> Vec<Zone> {
    let Some(root) = root() else {
        return Vec::new();
    };
    let countries = countries(&std::fs::read_to_string(root.join(COUNTRIES)).unwrap_or_default());
    let listed = std::fs::read_to_string(root.join(ZONES)).unwrap_or_default();
    let mut zones: Vec<Zone> = listed
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| {
            let mut columns = line.split('\t');
            let code = columns.next()?;
            let name = columns.nth(1)?;
            Zone::placed(name, countries.get(code).cloned())
        })
        .chain(Zone::named(UNIVERSAL))
        .collect();
    zones.sort_by(|left, right| left.name.cmp(&right.name));
    zones.dedup_by(|left, right| left.name == right.name);
    zones
}

fn countries(table: &str) -> HashMap<String, String> {
    table
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('\t'))
        .map(|(code, name)| (code.to_owned(), name.trim().to_owned()))
        .collect()
}

fn root() -> Option<PathBuf> {
    std::env::var_os(ROOT_ENV)
        .map(PathBuf::from)
        .into_iter()
        .chain(ROOTS.into_iter().map(PathBuf::from))
        .find(|root| root.join(ZONES).is_file())
}
