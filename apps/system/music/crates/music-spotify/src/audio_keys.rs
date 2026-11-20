use std::collections::HashMap;

use music_core::Result;
use music_spotify_audio::AudioKey;
use music_spotify_session::Session;
use music_spotify_wire::{FileId, SpotifyId};
use rikka_sync::{Mutex, MutexGuard};

use crate::failure;

pub(crate) struct AudioKeys {
    session: Session,
    known: Mutex<HashMap<FileId, AudioKey>>,
}

impl AudioKeys {
    pub(crate) fn new(session: Session) -> AudioKeys {
        AudioKeys {
            session,
            known: Mutex::default(),
        }
    }

    pub(crate) async fn key(&self, track: SpotifyId, file: FileId) -> Result<AudioKey> {
        let known = self.known().get(&file).copied();
        if let Some(key) = known {
            return Ok(key);
        }
        let key = self
            .session
            .audio_key(track, file)
            .await
            .map_err(failure::session)?;
        self.known().insert(file, key);
        Ok(key)
    }

    fn known(&self) -> MutexGuard<'_, HashMap<FileId, AudioKey>> {
        self.known.lock()
    }
}
