use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ServerCounts {
    #[serde(rename = "Countries")]
    pub countries: u32,
    #[serde(rename = "Servers")]
    pub servers: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamingService {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Icon", default)]
    pub icon: Option<String>,
    #[serde(rename = "ColoredIcon", default)]
    pub colored_icon: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct StreamingServices {
    #[serde(rename = "ResourceBaseURL")]
    pub resource_base_url: Option<String>,
    #[serde(rename = "StreamingServices")]
    pub countries: BTreeMap<String, BTreeMap<String, Vec<StreamingService>>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CityNames {
    #[serde(rename = "Language")]
    pub language: String,
    #[serde(rename = "Cities")]
    pub cities: BTreeMap<String, BTreeMap<String, Option<String>>>,
    #[serde(rename = "States")]
    pub states: BTreeMap<String, BTreeMap<String, Option<String>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserLocation {
    #[serde(rename = "IP")]
    pub ip: String,
    #[serde(rename = "Country", default)]
    pub country: String,
    #[serde(rename = "ISP", default)]
    pub isp: String,
    #[serde(rename = "Lat", default)]
    pub latitude: Option<f32>,
    #[serde(rename = "Long", default)]
    pub longitude: Option<f32>,
}

impl StreamingServices {
    #[must_use]
    pub fn icon_url(&self, icon: &str) -> Option<String> {
        let base = self.resource_base_url.as_deref()?;
        Some(format!("{}/{icon}", base.trim_end_matches('/')))
    }

    #[must_use]
    pub fn services(&self, country: &str, tier: u8) -> &[StreamingService] {
        self.countries
            .get(country)
            .and_then(|tiers| tiers.get(&tier.to_string()))
            .map_or(&[], Vec::as_slice)
    }
}

impl CityNames {
    #[must_use]
    pub fn city(&self, name: &str) -> Option<&str> {
        self.translation(&self.cities, name)
    }

    #[must_use]
    pub fn state(&self, name: &str) -> Option<&str> {
        self.translation(&self.states, name)
    }

    fn translation<'a>(
        &'a self,
        names: &'a BTreeMap<String, BTreeMap<String, Option<String>>>,
        name: &str,
    ) -> Option<&'a str> {
        names
            .values()
            .find_map(|country| country.get(name))
            .and_then(Option::as_deref)
    }
}
