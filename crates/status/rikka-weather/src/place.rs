use rikka_settings::session::weather::CHOSEN;

const EARTH_RADIUS: f64 = 6_371_000.0;

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Place {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country_code: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
}

impl Place {
    #[must_use]
    pub fn metres_to(&self, latitude: f64, longitude: f64) -> f64 {
        let (from, to) = (self.latitude.to_radians(), latitude.to_radians());
        let across = (latitude - self.latitude).to_radians();
        let along = (longitude - self.longitude).to_radians();
        let half =
            (across / 2.0).sin().powi(2) + from.cos() * to.cos() * (along / 2.0).sin().powi(2);
        2.0 * EARTH_RADIUS * half.sqrt().asin()
    }

    pub(crate) fn is_valid(&self) -> bool {
        (-90.0..=90.0).contains(&self.latitude) && (-180.0..=180.0).contains(&self.longitude)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Spot {
    #[default]
    Here,
    Fixed(Place),
}

impl Spot {
    #[must_use]
    pub fn from_setting(location: &str, place: Option<Place>) -> Spot {
        match place.filter(Place::is_valid) {
            Some(place) if location == CHOSEN => Spot::Fixed(place),
            _ => Spot::Here,
        }
    }
}
pub(crate) fn rounded(degrees: f64) -> f64 {
    (degrees * 100.0).round() / 100.0
}
