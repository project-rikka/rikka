use serde::Deserialize;

use crate::place::Place;

pub(crate) const URL: &str = "https://geocoding-api.open-meteo.com/v1/search";

pub(crate) fn query(name: &str, language: &str, count: usize) -> Vec<(&'static str, String)> {
    vec![
        ("name", name.to_owned()),
        ("count", count.clamp(1, 100).to_string()),
        ("language", language.to_owned()),
        ("format", "json".to_owned()),
    ]
}

#[derive(Debug, Deserialize)]
pub(crate) struct Answer {
    #[serde(default)]
    results: Vec<Found>,
}

#[derive(Debug, Deserialize)]
struct Found {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    country_code: Option<String>,
    admin1: Option<String>,
}

impl Answer {
    pub(crate) fn into_places(self) -> Vec<Place> {
        self.results
            .into_iter()
            .map(|found| Place {
                name: found.name,
                region: found.admin1.filter(|region| !region.is_empty()),
                country: found.country.filter(|country| !country.is_empty()),
                country_code: found.country_code.filter(|code| !code.is_empty()),
                latitude: found.latitude,
                longitude: found.longitude,
            })
            .filter(Place::is_valid)
            .collect()
    }
}
