use std::sync::Arc;

use rikka_gecko_protocol::Target;
use rikka_gecko_protocol::rikka::downloads::{
    CancelDownload, DownloadId, PauseDownload, RemoveDownload, ResumeDownload,
};

use crate::error::Result;
use crate::wire::Link;

#[derive(Clone, Debug)]
pub struct Downloads {
    link: Arc<Link>,
}

impl Downloads {
    pub(crate) fn new(link: &Arc<Link>) -> Downloads {
        Downloads {
            link: Arc::clone(link),
        }
    }

    pub async fn cancel(&self, id: &DownloadId) -> Result<()> {
        self.link
            .ask(Target::Runtime, &CancelDownload { id: id.clone() })
            .await
    }

    pub async fn pause(&self, id: &DownloadId) -> Result<()> {
        self.link
            .ask(Target::Runtime, &PauseDownload { id: id.clone() })
            .await
    }

    pub async fn resume(&self, id: &DownloadId) -> Result<()> {
        self.link
            .ask(Target::Runtime, &ResumeDownload { id: id.clone() })
            .await
    }

    pub async fn remove(&self, id: &DownloadId) -> Result<()> {
        self.link
            .ask(Target::Runtime, &RemoveDownload { id: id.clone() })
            .await
    }
}
