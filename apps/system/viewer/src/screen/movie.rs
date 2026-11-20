use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rikka_app::prelude::*;
use rikka_movie::Soundtrack;
use rikka_widgets::column;

use crate::bar;
use crate::screen::badge::Badge;
use crate::screen::player::Player;

#[derive(Debug, State)]
pub(crate) struct Movie {
    movie: rikka_movie::Movie,
    seen: Arc<Observers>,
}

impl Movie {
    pub(crate) fn open(path: &Path) -> Self {
        let movie = rikka_movie::Movie::open(path);
        let seen = Arc::new(Observers::new());
        let heard = Arc::clone(&seen);
        movie.on_change(move |_| heard.changed());
        Self { movie, seen }
    }

    pub(crate) fn stage(&self) -> Element {
        column! {
            container(video(self.movie.video()).fit(ImageFit::Contain).fill())
                .background(Color::BLACK)
                .grow(1.0);
            Element::new(Badge {
                resting: None,
                playing: self.is_playing(),
            })
            .position(0.0, 0.0)
            .fill();
        }
        .grow(1.0)
        .build()
    }
}

impl Player for Movie {
    fn volume(&self) -> Option<f32> {
        self.seen.read();
        matches!(self.movie.soundtrack(), Soundtrack::Open).then(|| self.movie.volume())
    }

    fn set_volume(&self, volume: f32) {
        self.movie.set_volume(volume);
    }

    fn is_playing(&self) -> bool {
        self.seen.read();
        self.movie.is_playing()
    }

    fn position(&self) -> Duration {
        self.seen.read();
        self.movie.position()
    }

    fn duration(&self) -> Option<Duration> {
        self.seen.read();
        self.movie.duration()
    }

    fn is_dead(&self) -> bool {
        self.seen.read();
        self.movie.picture() == VideoState::Failed
    }

    fn start(&mut self) {
        self.movie.play();
    }

    fn play_pause(&mut self) {
        match self.movie.is_playing() {
            true => self.movie.pause(),
            false => self.movie.play(),
        }
    }

    fn seek(&mut self, position: Duration) {
        self.movie.seek(position);
    }

    fn detail(&self) -> Option<Element> {
        if !self.is_dead() {
            return None;
        }
        let error = self
            .movie
            .video()
            .error()
            .map_or_else(|| tr!("viewer.video.undecodable"), Label::from);
        Some(bar::detail(error, palette().danger).build())
    }
}
