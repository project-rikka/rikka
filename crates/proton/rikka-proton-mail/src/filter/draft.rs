use serde::Serialize;

use crate::client::MailClient;
use crate::error::{Error, Result};
use crate::filter::FILTERS;
use crate::filter::saved::{DISABLED, ENABLED, Filter};
use crate::filter::simple::{SimpleFilter, WireSimpleFilter};
use crate::identifier::FilterId;

pub(crate) const SIEVE_VERSION: i32 = 2;

#[derive(Debug, Clone)]
enum Rule {
    Simple(SimpleFilter),
    Sieve(String),
}

#[must_use]
#[derive(Debug, Clone)]
pub struct FilterDraft {
    client: MailClient,
    name: String,
    rule: Option<Rule>,
    enabled: bool,
}

impl FilterDraft {
    pub(crate) fn new(client: MailClient, name: impl Into<String>) -> FilterDraft {
        FilterDraft {
            client,
            name: name.into(),
            rule: None,
            enabled: true,
        }
    }

    pub fn simple(mut self, filter: SimpleFilter) -> Self {
        self.rule = Some(Rule::Simple(filter));
        self
    }

    pub fn sieve(mut self, script: impl Into<String>) -> Self {
        self.rule = Some(Rule::Sieve(script.into()));
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub async fn create(self) -> Result<Filter> {
        let request = self.request()?;
        let response: Filter = self
            .client
            .api()
            .post(FILTERS)
            .json(&request)
            .send_field("Filter")
            .await?;
        Ok(response)
    }

    pub async fn update(self, id: &FilterId) -> Result<Filter> {
        let request = self.request()?;
        let response: Filter = self
            .client
            .api()
            .put(format!("{FILTERS}/{id}"))
            .json(&request)
            .send_field("Filter")
            .await?;
        Ok(response)
    }

    fn request(&self) -> Result<FilterRequest> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(Error::MissingFilterName);
        }

        let (sieve, simple) = match &self.rule {
            Some(Rule::Simple(filter)) => (filter.to_sieve(), Some(filter.to_wire())),
            Some(Rule::Sieve(script)) => (script.clone(), None),
            None => return Err(Error::MissingFilterRule),
        };

        Ok(FilterRequest {
            name: name.to_owned(),
            status: match self.enabled {
                true => ENABLED,
                false => DISABLED,
            },
            version: SIEVE_VERSION,
            sieve,
            simple,
        })
    }
}

#[derive(Serialize)]
struct FilterRequest {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Status")]
    status: i32,
    #[serde(rename = "Version")]
    version: i32,
    #[serde(rename = "Sieve")]
    sieve: String,
    #[serde(rename = "Simple", skip_serializing_if = "Option::is_none")]
    simple: Option<WireSimpleFilter>,
}
