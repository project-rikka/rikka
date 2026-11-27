use std::future::Future;
use std::sync::Arc;
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::json;

use crate::api::time::{self, millis};
use crate::channel::Link;
use crate::error::Result;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub title: String,
    #[serde(default, deserialize_with = "millis::optional::deserialize")]
    pub last_visit_time: Option<SystemTime>,
    #[serde(default)]
    pub visit_count: u32,
    #[serde(default)]
    pub typed_count: u32,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Visit {
    pub id: String,
    pub visit_id: String,
    #[serde(default, deserialize_with = "millis::optional::deserialize")]
    pub visit_time: Option<SystemTime>,
    #[serde(default)]
    pub transition: String,
}

#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct HistoryQuery {
    text: String,
    start: Option<SystemTime>,
    end: Option<SystemTime>,
    limit: u32,
}

impl HistoryQuery {
    const DEFAULT_LIMIT: u32 = 100;

    pub fn new(text: impl Into<String>) -> HistoryQuery {
        HistoryQuery {
            text: text.into(),
            start: None,
            end: None,
            limit: Self::DEFAULT_LIMIT,
        }
    }

    pub fn since(mut self, start: SystemTime) -> HistoryQuery {
        self.start = Some(start);
        self
    }

    pub fn until(mut self, end: SystemTime) -> HistoryQuery {
        self.end = Some(end);
        self
    }

    pub fn limit(mut self, limit: u32) -> HistoryQuery {
        self.limit = limit;
        self
    }
}

#[derive(Clone, Debug)]
pub struct History {
    link: Arc<Link>,
}

impl History {
    pub(crate) fn new(link: Arc<Link>) -> History {
        History { link }
    }

    pub fn search(
        &self,
        query: &HistoryQuery,
    ) -> impl Future<Output = Result<Vec<HistoryItem>>> + Send + use<> {
        let mut request = json!({
            "text": query.text,
            "startTime": query.start.map_or(0.0, time::to_millis),
            "maxResults": query.limit,
        });
        if let Some(end) = query.end {
            request["endTime"] = json!(time::to_millis(end));
        }
        self.link.call("history.search", vec![request]).decoded()
    }

    pub fn visits(&self, url: &str) -> impl Future<Output = Result<Vec<Visit>>> + Send + use<> {
        self.link
            .call("history.getVisits", vec![json!({ "url": url })])
            .decoded()
    }

    pub fn add(&self, url: &str) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call("history.addUrl", vec![json!({ "url": url })])
            .done()
    }

    pub fn delete_url(&self, url: &str) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call("history.deleteUrl", vec![json!({ "url": url })])
            .done()
    }

    pub fn delete_range(
        &self,
        start: SystemTime,
        end: SystemTime,
    ) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link
            .call(
                "history.deleteRange",
                vec![
                    json!({ "startTime": time::to_millis(start), "endTime": time::to_millis(end) }),
                ],
            )
            .done()
    }

    pub fn delete_all(&self) -> impl Future<Output = Result<()>> + Send + use<> {
        self.link.call("history.deleteAll", Vec::new()).done()
    }
}
