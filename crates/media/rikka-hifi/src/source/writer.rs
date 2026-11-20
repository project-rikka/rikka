use std::sync::Arc;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::source::feed::{Feed, Stopped};

pub struct PcmWriter {
    feed: Arc<Feed<f32>>,
}

impl PcmWriter {
    pub(crate) fn new(feed: Arc<Feed<f32>>) -> PcmWriter {
        PcmWriter { feed }
    }

    pub fn write(&self, samples: &[f32]) -> Result<()> {
        match self.feed.push(samples) {
            Ok(()) | Err(Stopped::Seeking) => Ok(()),
            Err(Stopped::Closed) => Err(Error::Closed),
        }
    }

    pub fn flush(&self, from: Duration) {
        self.feed.restart(from);
    }

    pub fn finish(&self) {
        self.feed.finish();
    }
}

impl Drop for PcmWriter {
    fn drop(&mut self) {
        self.feed.finish();
    }
}

impl std::fmt::Debug for PcmWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcmWriter").finish_non_exhaustive()
    }
}
