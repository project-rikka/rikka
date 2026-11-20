use std::collections::HashSet;
use std::time::{Duration, Instant};

use music_core::{Id, Result};
use music_spotify_api::SpClient;
use tokio::sync::Mutex;

use crate::uri::{self, PLAYLIST};
use crate::{playlists, worker};

const FRESH: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(crate) struct Followed {
    cached: Mutex<Option<(Instant, HashSet<String>)>>,
}

impl Followed {
    pub(crate) async fn contains(&self, api: &SpClient, playlist: &Id) -> Result<bool> {
        let playlist_uri = uri::compose(PLAYLIST, playlist);
        let mut cached = self.cached.lock().await;
        if let Some((_, listed)) = cached.as_ref().filter(|(read, _)| read.elapsed() < FRESH) {
            return Ok(listed.contains(&playlist_uri));
        }
        let api = api.clone();
        let listed = worker::run(async move { playlists::followed(&api).await }).await?;
        let is_followed = listed.contains(&playlist_uri);
        *cached = Some((Instant::now(), listed));
        Ok(is_followed)
    }

    pub(crate) async fn forget(&self) {
        *self.cached.lock().await = None;
    }
}
