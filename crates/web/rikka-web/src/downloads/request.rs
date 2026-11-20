use std::path::Path;

use rikka_gecko_protocol::rikka::downloads::{
    DownloadReason, DownloadRequest as WireRequest, SaveTo,
};

use crate::error::{Error, Result};
use crate::wire::Answer;

#[derive(Debug)]
pub struct DownloadRequest {
    pub uri: String,
    pub filename: String,
    pub mime_type: Option<String>,
    pub length: Option<u64>,
    pub reason: DownloadReason,
    answer: Answer,
}

impl DownloadRequest {
    pub(crate) fn from_wire(request: WireRequest, answer: Answer) -> DownloadRequest {
        DownloadRequest {
            uri: request.url,
            filename: request.filename,
            mime_type: request.mime_type,
            length: request.content_length,
            reason: request.reason,
            answer,
        }
    }

    pub fn save_to(self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let target = SaveTo::new(path).ok_or_else(|| Error::RelativePath(path.to_owned()))?;
        self.answer.with(&Some(target));
        Ok(())
    }

    pub fn cancel(self) {
        self.answer.with(&None::<SaveTo>);
    }
}
