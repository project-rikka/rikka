use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use rikka_gecko_protocol::rikka::downloads::{DownloadChanged, DownloadId, DownloadState};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Download {
    pub id: DownloadId,
    pub uri: String,
    pub referrer: Option<String>,
    pub private: bool,
    pub path: PathBuf,
    pub mime_type: Option<String>,
    pub state: DownloadState,
    pub received: u64,
    pub total: Option<u64>,
    pub started: Option<SystemTime>,
    pub error: Option<String>,
}

impl Download {
    pub(crate) fn from_wire(changed: DownloadChanged) -> Download {
        Download {
            id: changed.id,
            uri: changed.url,
            referrer: changed.referrer,
            private: changed.private,
            path: PathBuf::from(changed.path),
            mime_type: changed.mime_type,
            state: changed.state,
            received: changed.current_bytes,
            total: changed.total_bytes,
            started: changed
                .start_time
                .and_then(|millis| u64::try_from(millis).ok())
                .map(|millis| SystemTime::UNIX_EPOCH + Duration::from_millis(millis)),
            error: changed.error,
        }
    }
}
