use std::sync::Arc;

use music_core::{Id, Quality, Result, async_trait};
use music_stream::{Fetcher, Origin, Spool};

use crate::clients::Client;
use crate::innertube::Innertube;
use crate::solver::Solving;
use crate::stream;

const AHEAD: usize = 4;

pub(crate) struct YouTubeOrigin {
    tube: Arc<Innertube>,
    solving: Arc<Solving>,
    fetcher: Fetcher,
}

impl YouTubeOrigin {
    pub(crate) fn new(tube: Arc<Innertube>, solving: Arc<Solving>) -> YouTubeOrigin {
        YouTubeOrigin {
            tube,
            solving,
            fetcher: Fetcher::new(Client::VisionOs.agent()).ahead(AHEAD),
        }
    }
}

#[async_trait]
impl Origin for YouTubeOrigin {
    async fn fetch(&self, track: &Id, quality: Quality, spool: &mut Spool) -> Result<()> {
        let resolved = stream::resolve(&self.tube, &self.solving, track.as_str(), quality).await?;
        self.fetcher.fetch(stream::plan(&resolved), spool).await
    }
}
