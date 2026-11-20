use serde::Deserialize;

use crate::place::Place;

pub(crate) const REVERSE: &str = "https://nominatim.openstreetmap.org/reverse";
pub(crate) const SEARCH: &str = "https://nominatim.openstreetmap.org/search";

pub(crate) fn reverse(
    latitude: f64,
    longitude: f64,
    language: &str,
) -> Vec<(&'static str, String)> {
    vec![
        ("format", "jsonv2".to_owned()),
        ("lat", format!("{latitude:.2}")),
        ("lon", format!("{longitude:.2}")),
        ("zoom", "10".to_owned()),
        ("accept-language", language.to_owned()),
    ]
}

pub(crate) fn search(name: &str, language: &str, count: usize) -> Vec<(&'static str, String)> {
    vec![
        ("format", "jsonv2".to_owned()),
        ("q", name.to_owned()),
        ("limit", count.clamp(1, 40).to_string()),
        ("addressdetails", "1".to_owned()),
        ("featureType", "settlement".to_owned()),
        ("accept-language", language.to_owned()),
    ]
}

#[derive(Debug, Deserialize)]
pub(crate) struct Answer {
    name: Option<String>,
    lat: Option<String>,
    lon: Option<String>,
    address: Option<Address>,
}

#[derive(Debug, Default, Deserialize)]
struct Address {
    city: Option<String>,
    town: Option<String>,
    village: Option<String>,
    municipality: Option<String>,
    hamlet: Option<String>,
    county: Option<String>,
    state: Option<String>,
    province: Option<String>,
    country: Option<String>,
    country_code: Option<String>,
}

impl Answer {
    pub(crate) fn into_place(self, latitude: f64, longitude: f64) -> Option<Place> {
        let address = self.address.unwrap_or_default();
        let name = [
            address.city,
            address.town,
            address.village,
            address.municipality,
            address.hamlet,
            self.name,
            address.county.clone(),
        ]
        .into_iter()
        .flatten()
        .find(|name| !name.is_empty())?;
        Some(Place {
            region: address
                .state
                .or(address.province)
                .or(address.county)
                .filter(|region| *region != name),
            name,
            country: address.country,
            country_code: address.country_code.map(|code| code.to_ascii_uppercase()),
            latitude,
            longitude,
        })
    }

    pub(crate) fn into_found(self) -> Option<Place> {
        let latitude = self.lat.as_deref()?.parse().ok()?;
        let longitude = self.lon.as_deref()?.parse().ok()?;
        self.into_place(latitude, longitude).filter(Place::is_valid)
    }
}
