use rikka_proton_api::{Page, Paging};
use serde::de::DeserializeOwned;

use crate::client::{MailClient, PAGE};
use crate::error::Result;
use crate::identifier::LabelId;

const CACHED_FILTERS: [&str; 6] = ["LabelID", "AddressID", "Sort", "Desc", "Unread", "Limit"];

#[derive(Debug, Clone)]
pub(crate) struct Listing {
    client: MailClient,
    path: &'static str,
    pairs: Vec<(&'static str, String)>,
    page: u32,
    page_size: u32,
}

impl Listing {
    pub(crate) fn new(client: MailClient, path: &'static str) -> Listing {
        Listing {
            client,
            path,
            pairs: Vec::new(),
            page: 0,
            page_size: PAGE,
        }
    }

    pub(crate) fn page(mut self, page: u32) -> Listing {
        self.page = page;
        self
    }

    pub(crate) fn page_size(mut self, size: u32) -> Listing {
        self.page_size = size.clamp(1, PAGE);
        self
    }

    pub(crate) fn set(mut self, name: &'static str, value: String) -> Listing {
        self.pairs.retain(|(pair_name, _)| *pair_name != name);
        self.pairs.push((name, value));
        self
    }

    pub(crate) fn repeat(mut self, name: &'static str, value: String) -> Listing {
        self.pairs.push((name, value));
        self
    }

    pub(crate) fn client(&self) -> &MailClient {
        &self.client
    }

    pub(crate) fn cache_key(&self, all_pages: bool) -> String {
        let pairs: String = self
            .pairs
            .iter()
            .map(|(name, value)| format!("{}={name}&{}={value}&", name.len(), value.len()))
            .collect();
        let range = match all_pages {
            true => format!("from={}&all", self.page),
            false => format!("page={}&size={}", self.page, self.page_size),
        };
        format!("{}?{pairs}{range}", self.path)
    }

    pub(crate) fn is_cacheable(&self) -> bool {
        self.pairs
            .iter()
            .all(|(name, _)| CACHED_FILTERS.contains(name))
    }

    pub(crate) fn label(&self) -> Option<LabelId> {
        self.pairs
            .iter()
            .find(|(name, _)| *name == "LabelID")
            .map(|(_, value)| LabelId::from(value.as_str()))
    }

    pub(crate) async fn list<T: DeserializeOwned>(&self, field: &'static str) -> Result<Vec<T>> {
        let mut query = self.pairs.clone();
        query.push(("Page", self.page.to_string()));
        query.push(("PageSize", self.page_size.to_string()));

        Ok(self
            .client
            .api()
            .get(self.path)
            .query(&query)
            .send_field_or_default(field)
            .await?)
    }

    pub(crate) async fn all<T: DeserializeOwned>(&self, field: &'static str) -> Result<Vec<T>> {
        Ok(self
            .client
            .api()
            .get(self.path)
            .query(&self.pairs)
            .pages(Paging::numbered(self.page_size).starting_at(self.page))
            .field(field)
            .collect(Page::new)
            .await?)
    }
}
