use std::future::Future;
use std::time::Duration;

use rikka_llm::wire::{
    self, Completions, CompletionsDialect, Responses, ResponsesDialect, Transport,
};
use rikka_llm::{Error, Model, Result};
use rikka_net::{HeaderMap, StatusCode, Url};
use serde_json::Value;

use crate::checkup::Checkup;
use crate::credentials::EndpointCredentials;
use crate::listing::{self, Listing};
use crate::ollama::Ollama;

const COMPLETIONS: &str = "/chat/completions";
const RESPONSES: &str = "/responses";
const MODELS: &str = "/models";
pub(crate) const VERSIONED: &str = "/v1";

const LISTING_TIMEOUT: Duration = Duration::from_secs(30);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

const PING: &str = "Reply with the single word OK.";
const PING_TOKENS: u32 = 16;

const UNLISTED: [StatusCode; 3] = [
    StatusCode::NOT_FOUND,
    StatusCode::METHOD_NOT_ALLOWED,
    StatusCode::NOT_IMPLEMENTED,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wire {
    #[default]
    Completions,
    Responses,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Endpoint {
    base: String,
    #[serde(default)]
    wire: Wire,
}

impl Endpoint {
    pub fn new(base: &str) -> Result<Endpoint> {
        Ok(Endpoint {
            base: normalized(base)?,
            wire: Wire::default(),
        })
    }

    #[must_use]
    pub fn with_wire(mut self, wire: Wire) -> Endpoint {
        self.wire = wire;
        self
    }

    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    #[must_use]
    pub fn wire(&self) -> Wire {
        self.wire
    }

    pub fn connect(&self, credentials: &EndpointCredentials, model: &str) -> Result<Model> {
        if model.trim().is_empty() {
            return Err(Error::Rejected("no model was named".to_owned()));
        }
        let headers = credentials.signed().json().build()?;
        Ok(match self.wire {
            Wire::Completions => Completions::new(
                Transport::new(self.under(COMPLETIONS)?, headers)?,
                model,
                CompletionsDialect::Compatible,
            )
            .legacy_max_tokens()
            .build(),
            Wire::Responses => Responses::new(
                Transport::new(self.under(RESPONSES)?, headers)?,
                model,
                ResponsesDialect::Compatible,
            )
            .build(),
        })
    }

    pub fn models(
        &self,
        credentials: &EndpointCredentials,
    ) -> impl Future<Output = Result<Listing>> + Send + 'static {
        let asked = self.under(MODELS).and_then(|url| {
            Ok((
                url,
                credentials.signed().accept("application/json").build()?,
                credentials.signed().json().build()?,
            ))
        });
        let base = self.base.clone();
        async move {
            let (url, headers, described) = asked?;
            let Listing::Models(models) = listed(url, headers).await? else {
                return Ok(Listing::Unlisted);
            };
            Ok(Listing::Models(
                match Ollama::recognize(&base, described).await? {
                    Some(ollama) => ollama.described(models).await,
                    None => models,
                },
            ))
        }
    }

    pub fn check(
        &self,
        credentials: &EndpointCredentials,
        model: Option<&str>,
    ) -> impl Future<Output = Result<Checkup>> + Send + 'static {
        let listing = self.models(credentials);
        let pinged = model
            .filter(|model| !model.trim().is_empty())
            .map(|model| {
                self.connect(credentials, model)
                    .map(|connected| (model.to_owned(), connected))
            })
            .transpose();
        async move {
            let listing = listing.await?;
            let Some((named, model)) = pinged? else {
                return Ok(Checkup::new(listing, None));
            };
            model
                .chat()
                .user(PING)
                .max_tokens(PING_TOKENS)
                .stream()
                .await?
                .response()
                .await?;
            Ok(Checkup::new(listing, Some(named)))
        }
    }

    fn under(&self, path: &str) -> Result<Url> {
        wire::endpoint_under(&self.base, path)
    }
}

async fn listed(url: Url, headers: HeaderMap) -> Result<Listing> {
    let sent = wire::client(LISTING_TIMEOUT, CONNECT_TIMEOUT)?
        .get(url)
        .headers(headers)
        .send()
        .await
        .map_err(wire::translate)?;
    let answered = match sent.error_for_status().await {
        Ok(answered) => answered,
        Err(refused)
            if refused
                .status()
                .is_some_and(|status| UNLISTED.contains(&status)) =>
        {
            return Ok(Listing::Unlisted);
        }
        Err(refused) => return Err(wire::translate(refused)),
    };
    let body: Value = answered.json().await.map_err(wire::translate)?;
    listing::read(&body)
}

fn normalized(typed: &str) -> Result<String> {
    let typed = typed.trim();
    let invalid = || Error::InvalidEndpoint(typed.to_owned());
    let mut url = Url::parse(typed).map_err(|_| invalid())?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none_or(str::is_empty)
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid());
    }
    url.set_query(None);
    url.set_fragment(None);

    let mut path = url.path().trim_end_matches('/');
    if let Some(stripped) = [COMPLETIONS, RESPONSES, MODELS]
        .iter()
        .find_map(|suffix| path.strip_suffix(suffix))
    {
        path = stripped.trim_end_matches('/');
    }
    let path = match path {
        "" => VERSIONED.to_owned(),
        kept => kept.to_owned(),
    };
    url.set_path(&path);
    Ok(url.as_str().trim_end_matches('/').to_owned())
}
