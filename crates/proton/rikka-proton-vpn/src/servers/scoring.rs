use crate::servers::catalogue::UserLocation;
use crate::servers::logicals::{LogicalServer, ServerLocation};
use crate::servers::status::{BinaryStatus, ServerStatus};
use rikka_proton_api::delay::random_unit;

const NORMALIZATION: f64 = 10_000.0;
const BANDWIDTH_PER_KILOMETRE: f64 = 738_000.0;
const PARTIAL_SCORE_CEILING: f64 = 0.99;
const JITTER_RANGE: f64 = -(100.0 / NORMALIZATION);
const EARTH_RADIUS_KM: f64 = 6_371.0;
const UNAVAILABLE_PENALTY: f64 = 1_000.0;
const ABROAD_PENALTY: f64 = 1.0;
const COSTLY_ABROAD_PENALTY: f64 = 3.0;

#[must_use]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scoring {
    location: Option<ServerLocation>,
    country: Option<String>,
    jitter_disabled: bool,
}

impl Scoring {
    pub fn new() -> Scoring {
        Scoring::default()
    }

    pub fn for_user(user: &UserLocation) -> Scoring {
        let mut scoring = Scoring::new();
        if let (Some(latitude), Some(longitude)) = (user.latitude, user.longitude) {
            scoring = scoring.with_location(ServerLocation {
                latitude,
                longitude,
            });
        }
        match user.country.is_empty() {
            true => scoring,
            false => scoring.with_country(&user.country),
        }
    }

    pub fn with_location(mut self, location: ServerLocation) -> Scoring {
        self.location = Some(location);
        self
    }

    pub fn with_country(mut self, country: impl AsRef<str>) -> Scoring {
        self.country = Some(country_code(country.as_ref()));
        self
    }

    pub fn without_jitter(mut self) -> Scoring {
        self.jitter_disabled = true;
        self
    }

    pub fn apply(&self, status: &BinaryStatus, servers: &mut [LogicalServer]) {
        for server in servers {
            let (Some(reference), Some(entry), Some(exit)) = (
                server.status_reference,
                server.entry_location,
                server.exit_location,
            ) else {
                continue;
            };
            let server_status = status.get_or_unknown(reference.index);
            let score = self.score(server, &server_status, entry, exit);

            server.status = Some(u8::from(server_status.is_enabled()));
            server.visible = server_status.is_visible();
            server.load = Some(u32::from(server_status.load()));
            server.score = Some(score);
        }
    }

    fn score(
        &self,
        server: &LogicalServer,
        status: &ServerStatus,
        entry: ServerLocation,
        exit: ServerLocation,
    ) -> f64 {
        let partial = f64::from(status.partial_score());
        let distance = self.location.map_or(0.0, |user| {
            haversine_km(user, entry) + haversine_km(entry, exit)
        });
        let proximity = normalize(BANDWIDTH_PER_KILOMETRE / distance.max(1.0));
        let base = (proximity.max(partial) + self.jitter()).clamp(0.0, 1.0);

        base + self.penalty(server, status, partial)
    }

    fn penalty(&self, server: &LogicalServer, status: &ServerStatus, partial: f64) -> f64 {
        let reference = server.status_reference.unwrap_or_default();
        let at_home = self
            .country
            .as_deref()
            .is_none_or(|country| country == country_code(&server.exit_country));

        let mut penalty = reference.penalty;
        if !status.is_enabled() || !status.is_visible() {
            penalty += UNAVAILABLE_PENALTY;
        }
        if !at_home || partial >= PARTIAL_SCORE_CEILING {
            penalty += ABROAD_PENALTY;
        }
        if !at_home && reference.cost == 1 {
            penalty += COSTLY_ABROAD_PENALTY;
        }
        penalty
    }

    fn jitter(&self) -> f64 {
        if self.jitter_disabled {
            return 0.0;
        }
        (random_unit() - 0.5) * JITTER_RANGE
    }
}

pub(crate) fn country_code(country: &str) -> String {
    let upper = country.trim().to_ascii_uppercase();
    match upper.as_str() {
        "UK" => "GB".to_owned(),
        _ => upper,
    }
}

fn normalize(bandwidth: f64) -> f64 {
    (NORMALIZATION - bandwidth) / NORMALIZATION
}

fn haversine_km(from: ServerLocation, to: ServerLocation) -> f64 {
    let radians = std::f64::consts::PI / 180.0;
    let (from_latitude, from_longitude) = (
        f64::from(from.latitude) * radians,
        f64::from(from.longitude) * radians,
    );
    let (to_latitude, to_longitude) = (
        f64::from(to.latitude) * radians,
        f64::from(to.longitude) * radians,
    );
    let latitude = (to_latitude - from_latitude) / 2.0;
    let longitude = (to_longitude - from_longitude) / 2.0;
    let haversine =
        latitude.sin().powi(2) + from_latitude.cos() * to_latitude.cos() * longitude.sin().powi(2);

    EARTH_RADIUS_KM * 2.0 * haversine.sqrt().asin()
}
