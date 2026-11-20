use std::time::Duration;

use rikka_net::{Client, RetryPolicy};

use crate::engine::SearchEngine;
use crate::error::{Error, Result};
use crate::suggestion::{self, SearchSuggestion};

pub const SUGGEST_DEBOUNCE: Duration = Duration::from_millis(100);

const TIMEOUT: Duration = Duration::from_millis(1500);
const RESPONSE_LIMIT: usize = 64 * 1024;
const MAX_TERMS: usize = 512;

#[derive(Debug, Clone)]
pub struct Suggester {
    client: Client,
}

#[derive(Debug, Clone)]
#[must_use]
pub struct SuggesterBuilder {
    language: Option<String>,
    timeout: Duration,
}

impl Suggester {
    pub fn new() -> Result<Suggester> {
        Suggester::builder().build()
    }

    pub fn builder() -> SuggesterBuilder {
        SuggesterBuilder {
            language: None,
            timeout: TIMEOUT,
        }
    }

    pub async fn suggest(
        &self,
        engine: &SearchEngine,
        terms: &str,
    ) -> Result<Vec<SearchSuggestion>> {
        let terms = terms.trim();
        if terms.is_empty() || terms.len() > MAX_TERMS {
            return Ok(Vec::new());
        }
        let Some(url) = engine.suggestion_url(terms) else {
            return Ok(Vec::new());
        };
        let response = self.client.get(url).send().await?;
        if !response.is_success() {
            return Err(Error::Status(response.status().as_u16()));
        }
        let body = response.bytes_limited(RESPONSE_LIMIT).await?;
        suggestion::parse(engine.suggestion_format(), &body)
    }
}

impl SuggesterBuilder {
    pub fn accept_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn build(self) -> Result<Suggester> {
        let mut builder = Client::builder()
            .cookies(false)
            .referer(false)
            .retry(RetryPolicy::never())
            .timeout(self.timeout)
            .connect_timeout(self.timeout)
            .header(
                "accept",
                "application/json, text/javascript;q=0.9, */*;q=0.1",
            );
        if let Some(language) = self.language {
            builder = builder.header("accept-language", language);
        }
        let client = builder.build()?;
        Ok(Suggester { client })
    }
}
