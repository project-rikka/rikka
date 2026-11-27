use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::json;

use crate::api::time::iso;
use crate::channel::Link;
use crate::error::Result;
use crate::ids::DownloadId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    InProgress,
    Interrupted,
    Complete,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadItem {
    pub id: DownloadId,
    pub url: String,
    #[serde(default)]
    pub final_url: String,
    #[serde(default)]
    pub filename: PathBuf,
    #[serde(default)]
    pub mime: String,
    pub state: DownloadState,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub can_resume: bool,
    pub error: Option<String>,
    #[serde(default)]
    pub bytes_received: f64,
    #[serde(default)]
    pub total_bytes: f64,
    #[serde(default)]
    pub exists: bool,
    #[serde(default, deserialize_with = "iso::deserialize")]
    pub start_time: Option<SystemTime>,
    #[serde(default, deserialize_with = "iso::deserialize")]
    pub end_time: Option<SystemTime>,
}

impl DownloadItem {
    #[must_use]
    pub fn progress(&self) -> Option<f64> {
        (self.total_bytes > 0.0).then(|| (self.bytes_received / self.total_bytes).clamp(0.0, 1.0))
    }
}

#[derive(Clone, Debug)]
pub struct Downloads {
    link: Arc<Link>,
}

impl Downloads {
    pub(crate) fn new(link: Arc<Link>) -> Downloads {
        Downloads { link }
    }

    pub fn start(
        &self,
        url: &str,
        ask_where: bool,
    ) -> impl Future<Output = Result<DownloadId>> + Send + use<> {
        self.link
            .call(
                "downloads.download",
                vec![json!({ "url": url, "saveAs": ask_where })],
            )
            .decoded()
    }

    pub fn pause(&self, id: DownloadId) {
        self.link.notify("downloads.pause", vec![json!(id)]);
    }

    pub fn resume(&self, id: DownloadId) {
        self.link.notify("downloads.resume", vec![json!(id)]);
    }

    pub fn cancel(&self, id: DownloadId) {
        self.link.notify("downloads.cancel", vec![json!(id)]);
    }

    pub fn erase(&self, id: DownloadId) {
        self.link
            .notify("downloads.erase", vec![json!({ "id": id })]);
    }

    pub fn delete_file(&self, id: DownloadId) {
        self.link.notify("downloads.removeFile", vec![json!(id)]);
    }

    pub fn open(&self, id: DownloadId) {
        self.link.notify("downloads.open", vec![json!(id)]);
    }

    pub fn show(&self, id: DownloadId) {
        self.link.notify("downloads.show", vec![json!(id)]);
    }

    pub fn hide_chrome_bubble(&self) {
        self.link
            .notify("downloads.setUiOptions", vec![json!({ "enabled": false })]);
    }
}
