use std::path::PathBuf;
use std::time::Duration;

use http::StatusCode;
use url::Url;

use crate::checksum::Checksum;

#[derive(Debug, Clone)]
pub struct DownloadSummary {
    pub path: PathBuf,
    pub url: Url,
    pub status: StatusCode,
    pub bytes: u64,
    pub resumed: bool,
    pub skipped: bool,
    pub elapsed: Duration,
    pub checksum: Option<Checksum>,
}

impl DownloadSummary {
    #[must_use]
    pub fn bytes_per_second(&self) -> f64 {
        let seconds = self.elapsed.as_secs_f64();
        if seconds <= 0.0 {
            return 0.0;
        }

        #[expect(
            clippy::cast_precision_loss,
            reason = "a transfer rate needs no more precision"
        )]
        let bytes = self.bytes as f64;
        bytes / seconds
    }
}
