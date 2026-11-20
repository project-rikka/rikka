use music_core::AudioCache;
use rikka_app::prelude::*;

use crate::cache_size::CacheSize;
use crate::music::Music;

impl Music {
    pub(crate) fn audio(&self) -> &AudioCache {
        &self.audio
    }

    pub(crate) fn stored(&self) -> u64 {
        *self.stored
    }

    pub(crate) fn set_cache_size(&mut self, size: CacheSize) {
        if self.preferences.cache() == size {
            return;
        }
        self.preferences.set_cache(size);
        self.audio.set_limit(size.bytes());
        self.stored.set(*self.stored + 1);
    }

    pub(crate) fn clear_cache(&mut self) {
        let audio = self.audio.clone();
        spawn_blocking(
            move || audio.clear().map_err(|error| error.to_string()),
            |music: &mut Music, cleared: Result<(), String>| music.cache_cleared(cleared),
        );
    }

    pub(crate) fn cache_cleared(&mut self, cleared: Result<(), String>) {
        if let Err(reason) = cleared {
            self.error_message.set(Some(reason));
        }
        self.stored.set(*self.stored + 1);
    }
}
