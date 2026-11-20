use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rikka_app::prelude::*;

use crate::screen::Screen;
use crate::screen::badge::Badge;
use crate::screen::player::Player;
use crate::screen::sound::{self, Opened, Sound};
use rikka_audio::Track;

#[derive(Debug)]
pub(crate) struct Music {
    path: PathBuf,
    sound: Sound,
    seen: Arc<Observers>,
}

impl Music {
    pub(crate) fn open(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            sound: Sound::Opening,
            seen: Arc::new(Observers::new()),
        }
    }

    pub(crate) fn stage(&self) -> Element {
        if let Sound::Unplayable(error) = &self.sound {
            return status_page(
                MaterialSymbol::VOLUME_OFF,
                tr!(
                    "viewer.audio.unplayable",
                    reason = error.to_string().as_str()
                ),
            )
            .grow(1.0)
            .build();
        }
        Element::new(Badge {
            resting: Some(MaterialSymbol::MUSIC_NOTE),
            playing: self.is_playing(),
        })
        .grow(1.0)
    }

    fn opened(screen: &mut Screen, path: &Path, opened: Opened) {
        let Some(music) = screen.music().filter(|music| music.path == path) else {
            return;
        };
        music.sound = Sound::from(opened);
        let heard = Arc::clone(&music.seen);
        if let Some(track) = music.sound.track_mut() {
            track.on_change(move |_| heard.changed());
        }
    }
}

impl Player for Music {
    fn volume(&self) -> Option<f32> {
        self.seen.read();
        self.sound.track().map(Track::volume)
    }

    fn set_volume(&self, volume: f32) {
        if let Some(sound) = self.sound.track() {
            sound.set_volume(volume);
        }
    }

    fn is_playing(&self) -> bool {
        self.seen.read();
        self.sound.track().is_some_and(Track::is_playing)
    }

    fn position(&self) -> Duration {
        self.seen.read();
        self.sound.track().map(Track::position).unwrap_or_default()
    }

    fn duration(&self) -> Option<Duration> {
        self.seen.read();
        self.sound.track().and_then(Track::duration)
    }

    fn is_dead(&self) -> bool {
        self.seen.read();
        self.sound.track().is_none()
    }

    fn start(&mut self) {
        sound::stream(&self.path, Music::opened);
    }

    fn play_pause(&mut self) {
        let Some(sound) = self.sound.track_mut() else {
            return;
        };
        match sound.is_playing() {
            true => sound.pause(),
            false => sound.play(),
        }
    }

    fn seek(&mut self, position: Duration) {
        if let Some(sound) = self.sound.track() {
            sound.seek(position);
        }
    }
}
