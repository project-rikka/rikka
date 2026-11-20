use std::time::Duration;

use serde::de::DeserializeOwned;

use crate::error::{Error, Result};
use crate::nominatim;
use crate::open_meteo::{air, forecast, geocoding};
use crate::place::Place;

const USER_AGENT: &str = concat!(
    "Rikka/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/project-rikka/rikka)"
);
const PATIENCE: Duration = Duration::from_secs(20);
const FOUND: usize = 10;

#[derive(Debug)]
pub(crate) struct Fetcher {
    client: rikka_net::Client,
}

impl Fetcher {
    pub(crate) fn new() -> Result<Fetcher> {
        let client = rikka_net::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(PATIENCE)
            .build()?;
        Ok(Fetcher { client })
    }

    async fn text(&self, url: &str, query: &[(&'static str, String)]) -> Result<String> {
        Ok(self
            .client
            .get(url)
            .query(query)
            .send()
            .await?
            .error_for_status()
            .await?
            .text()
            .await?)
    }

    pub(crate) async fn forecast(&self, latitude: f64, longitude: f64) -> Result<String> {
        self.text(forecast::URL, &forecast::query(latitude, longitude))
            .await
    }

    pub(crate) async fn air(&self, latitude: f64, longitude: f64) -> Result<String> {
        self.text(air::URL, &air::query(latitude, longitude)).await
    }

    pub(crate) async fn reverse(
        &self,
        latitude: f64,
        longitude: f64,
        language: &str,
    ) -> Result<String> {
        self.text(
            nominatim::REVERSE,
            &nominatim::reverse(latitude, longitude, language),
        )
        .await
    }

    async fn places(&self, name: &str, language: &str) -> Result<Vec<Place>> {
        let body = self
            .text(geocoding::URL, &geocoding::query(name, language, FOUND))
            .await?;
        let places = parsed::<geocoding::Answer>(&body)?.into_places();
        if !places.is_empty() {
            return Ok(places);
        }
        let body = self
            .text(nominatim::SEARCH, &nominatim::search(name, language, FOUND))
            .await?;
        Ok(parsed::<Vec<nominatim::Answer>>(&body)?
            .into_iter()
            .filter_map(nominatim::Answer::into_found)
            .collect())
    }
}

pub(crate) fn parsed<T: DeserializeOwned>(body: &str) -> Result<T> {
    serde_json::from_str(body).map_err(|error| Error::Unreadable(error.to_string()))
}

pub async fn search(name: &str, language: &str) -> Result<Vec<Place>> {
    let name = name.trim();
    if name.is_empty() {
        return Ok(Vec::new());
    }
    Fetcher::new()?.places(name, language).await
}
