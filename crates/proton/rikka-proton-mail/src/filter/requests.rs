use serde::Serialize;

use crate::client::MailClient;
use crate::error::Result;
use crate::filter::FILTERS;
use crate::filter::draft::{FilterDraft, SIEVE_VERSION};
use crate::filter::issue::SieveIssue;
use crate::filter::saved::Filter;
use crate::identifier::FilterId;

impl MailClient {
    pub async fn filters(&self) -> Result<Vec<Filter>> {
        Ok(self
            .api()
            .get(FILTERS)
            .send_field_or_default("Filters")
            .await?)
    }

    pub async fn filter(&self, id: &FilterId) -> Result<Filter> {
        let response: Filter = self
            .api()
            .get(format!("{FILTERS}/{id}"))
            .send_field("Filter")
            .await?;
        Ok(response)
    }

    pub fn new_filter(&self, name: impl Into<String>) -> FilterDraft {
        FilterDraft::new(self.clone(), name)
    }

    pub async fn delete_filter(&self, id: &FilterId) -> Result<()> {
        self.api()
            .delete(format!("{FILTERS}/{id}"))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn set_filter_enabled(&self, id: &FilterId, enabled: bool) -> Result<()> {
        let action = match enabled {
            true => "enable",
            false => "disable",
        };
        self.api()
            .put(format!("{FILTERS}/{id}/{action}"))
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn order_filters(&self, ids: &[FilterId]) -> Result<()> {
        self.api()
            .put(format!("{FILTERS}/order"))
            .json(&OrderRequest { ids })
            .send_unit()
            .await?;
        Ok(())
    }

    pub async fn check_sieve(&self, script: &str) -> Result<Vec<SieveIssue>> {
        Ok(self
            .api()
            .put(format!("{FILTERS}/check"))
            .json(&CheckRequest {
                sieve: script,
                version: SIEVE_VERSION,
            })
            .send_field_or_default("Issues")
            .await?)
    }
}

#[derive(Serialize)]
struct OrderRequest<'a> {
    #[serde(rename = "FilterIDs")]
    ids: &'a [FilterId],
}

#[derive(Serialize)]
struct CheckRequest<'a> {
    #[serde(rename = "Sieve")]
    sieve: &'a str,
    #[serde(rename = "Version")]
    version: i32,
}
