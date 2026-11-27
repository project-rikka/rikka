use std::collections::HashMap;

use rikka_net::Url;

use crate::error::{Error, Result};
use crate::transport;

const GOOGLE: &str = "https://android.clients.google.com/auth";
const AGENT: &str = "GoogleAuth/1.4 (Pixel 8 Build/UQ1A.240205.004)";
const OK: u16 = 200;
const FAILURE: &str = "Error";

pub(crate) const SIGNATURE: &str = "38918a453d07199354f8b19af05ec6562ced5788";
pub(crate) const PLAY_SERVICES: &str = "240913000";
pub(crate) const SDK: &str = "34";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    url: Url,
}

#[derive(Debug)]
pub(crate) struct Fields(HashMap<String, String>);

impl Endpoint {
    #[must_use]
    pub fn google() -> Endpoint {
        Endpoint {
            url: Url::parse(GOOGLE).expect("the Google endpoint is a URL"),
        }
    }

    pub fn new(url: &str) -> Result<Endpoint> {
        let url = Url::parse(url).map_err(|error| Error::Endpoint(error.to_string()))?;
        match url.scheme() {
            "http" | "https" => Ok(Endpoint { url }),
            scheme => Err(Error::Endpoint(format!("unsupported scheme {scheme}"))),
        }
    }

    pub(crate) async fn ask(&self, fields: &[(&str, &str)]) -> Result<Fields> {
        let mut form = self.url.clone();
        form.query_pairs_mut().clear().extend_pairs(fields);
        let form = form.query().unwrap_or_default().to_owned();
        let url = self.url.clone();
        let answer = tokio::task::spawn_blocking(move || transport::post_form(&url, AGENT, &form))
            .await
            .map_err(|error| Error::Unreachable(error.to_string()))??;
        let fields = Fields::parse(&answer.body);
        match fields.get(FAILURE) {
            Some(reason) => Err(Error::refusal(reason)),
            None if answer.status != OK => Err(Error::Refused(format!("status {}", answer.status))),
            None => Ok(fields),
        }
    }
}

impl Default for Endpoint {
    fn default() -> Endpoint {
        Endpoint::google()
    }
}

impl Fields {
    fn parse(body: &str) -> Fields {
        Fields(
            body.lines()
                .filter_map(|line| line.trim_end_matches('\r').split_once('='))
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
        )
    }

    pub(crate) fn get(&self, name: &str) -> Option<&str> {
        self.0
            .get(name)
            .map(String::as_str)
            .filter(|value| !value.is_empty())
    }

    pub(crate) fn need(&self, name: &'static str) -> Result<&str> {
        self.get(name).ok_or(Error::Malformed(name))
    }
}
