use std::path::Path;

use http::{HeaderMap, header};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ResumeState {
    url: String,
    etag: Option<String>,
    last_modified: Option<String>,
    total: Option<u64>,
}

impl ResumeState {
    pub(crate) fn new(url: &Url, headers: &HeaderMap, total: Option<u64>) -> ResumeState {
        ResumeState {
            url: url.as_str().to_owned(),
            etag: header_text(headers, header::ETAG),
            last_modified: header_text(headers, header::LAST_MODIFIED),
            total,
        }
    }

    pub(crate) async fn load(path: &Path) -> Option<ResumeState> {
        let bytes = tokio::fs::read(path).await.ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    pub(crate) async fn save(&self, path: &Path) -> Result<()> {
        let bytes = serde_json::to_vec(self).map_err(|source| Error::Encode {
            format: "JSON",
            source: source.into(),
        })?;
        tokio::fs::write(path, bytes).await?;
        Ok(())
    }

    pub(crate) fn is_for(&self, url: &Url) -> bool {
        self.url == url.as_str()
    }

    pub(crate) fn validator(&self) -> Option<&str> {
        self.etag
            .as_deref()
            .filter(|tag| !tag.starts_with("W/"))
            .or(self.last_modified.as_deref())
    }

    pub(crate) fn total(&self) -> Option<u64> {
        self.total
    }
}

fn header_text(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)?
        .to_str()
        .ok()
        .map(std::borrow::ToOwned::to_owned)
}
