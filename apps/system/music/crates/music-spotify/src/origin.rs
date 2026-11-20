use std::sync::Arc;

use futures_util::future::join;
use music_core::{Error, Id, Quality, Result, async_trait};
use music_spotify_api::{SpClient, Track as TrackMessage};
use music_spotify_audio::{Decrypted, Label};
use music_spotify_session::Session;
use music_spotify_wire::{FileId, SpotifyId};
use music_stream::{Origin, Spool, Stored};
use rikka_audio_decode::StreamReader;

use crate::audio_file::Listener;
use crate::audio_keys::AudioKeys;
use crate::cdn::{self, Cdn};
use crate::failure::unreadable;
use crate::metadata;
use crate::uri::{self, TRACK};
use crate::worker;

pub(crate) struct SpotifyOrigin {
    session: Session,
    api: SpClient,
    keys: Arc<AudioKeys>,
    cdn: Cdn,
}

struct Located {
    label: Label,
    addresses: Vec<String>,
}

impl SpotifyOrigin {
    pub(crate) fn new(session: Session, api: SpClient) -> SpotifyOrigin {
        SpotifyOrigin {
            keys: Arc::new(AudioKeys::new(session.clone())),
            session,
            api,
            cdn: Cdn::new(),
        }
    }
}

#[async_trait]
impl Origin for SpotifyOrigin {
    async fn fetch(&self, track: &Id, quality: Quality, spool: &mut Spool) -> Result<()> {
        let locating = locate(
            self.session.clone(),
            self.api.clone(),
            Arc::clone(&self.keys),
            track.clone(),
            quality,
        );
        let located = worker::run(locating).await?;
        let plan = self.cdn.plan(&located.addresses).await?;
        self.cdn
            .download(plan.lead(located.label.encode()), spool)
            .await
    }

    fn open(&self, mut stored: Stored) -> Result<Box<dyn StreamReader>> {
        let label = Label::read(&mut stored).map_err(unreadable)?;
        let keys = Arc::clone(&self.keys);
        let key = worker::wait(async move {
            let track = SpotifyId::from_gid(&label.track).map_err(unreadable)?;
            keys.key(track, FileId::new(label.file)).await
        })??;
        Ok(Box::new(Decrypted::new(Box::new(stored), &key)))
    }
}

async fn locate(
    session: Session,
    api: SpClient,
    keys: Arc<AudioKeys>,
    track: Id,
    quality: Quality,
) -> Result<Located> {
    let spotify = uri::spotify_id(&track)?;
    let message = metadata::message::<TrackMessage>(&api, &uri::compose(TRACK, &track))
        .await?
        .ok_or_else(|| Error::Missing(format!("track {track}")))?;
    let file = Listener::of(&session.account())
        .file(&message, quality)
        .ok_or_else(|| Error::Missing(format!("a stream of {track}")))?;
    let (key, addresses) = join(keys.key(spotify, file), cdn::addresses(&api, file)).await;
    key?;
    Ok(Located {
        label: Label {
            track: spotify.to_gid(),
            file: *file.as_bytes(),
        },
        addresses: addresses?,
    })
}
