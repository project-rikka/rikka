use std::path::PathBuf;

use music_core::{Error, Id, Quality, Result, async_trait};
use rikka_audio_decode::StreamReader;
use rikka_hifi::Gains;

use crate::spool::{Spool, Stored};

#[derive(Clone, Debug, PartialEq)]
pub struct Located {
    pub path: PathBuf,
    pub gains: Gains,
}

#[async_trait]
pub trait Origin: Send + Sync + 'static {
    async fn fetch(&self, track: &Id, _quality: Quality, _spool: &mut Spool) -> Result<()> {
        Err(Error::Missing(format!("a stream of {track}")))
    }

    fn open(&self, stored: Stored) -> Result<Box<dyn StreamReader>> {
        Ok(Box::new(stored))
    }

    fn locate(&self, _track: &Id) -> Option<Located> {
        None
    }

    async fn gains(&self, _track: &Id) -> Gains {
        Gains::default()
    }
}
