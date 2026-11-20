use std::path::Path;
use std::time::Duration;

use futures_util::Stream;
use rikka_movie::Movie;
use rikka_types::Size;
use rikka_ui::{VideoController, VideoState};
use rikka_worker::Observable;

#[derive(Debug)]
pub(crate) struct Preview {
    movie: Movie,
}

impl Preview {
    #[must_use]
    pub(crate) fn open(file: &Path) -> Preview {
        Preview {
            movie: Movie::open(file),
        }
    }

    pub(crate) fn changes(&self) -> impl Stream<Item = ()> + Send + 'static {
        let moved = Observable::default();
        let heard = moved.clone();
        self.movie.on_change(move |_| heard.set(()));
        moved.changes()
    }

    #[must_use]
    pub(crate) fn video(&self) -> &VideoController {
        self.movie.video()
    }

    #[must_use]
    pub(crate) fn length(&self) -> Option<Duration> {
        self.movie.video().duration()
    }

    #[must_use]
    pub(crate) fn shape(&self) -> Option<Size> {
        self.movie.video().size()
    }

    #[must_use]
    pub(crate) fn position(&self) -> Duration {
        self.movie.position()
    }

    #[must_use]
    pub(crate) fn is_playing(&self) -> bool {
        self.movie.is_playing()
    }

    #[must_use]
    pub(crate) fn is_open(&self) -> bool {
        self.movie.picture() != VideoState::Opening
    }

    #[must_use]
    pub(crate) fn trouble(&self) -> Option<String> {
        (self.movie.picture() == VideoState::Failed).then(|| {
            self.movie
                .video()
                .error()
                .unwrap_or_else(|| "nothing here can decode it".to_owned())
        })
    }

    pub(crate) fn moved_to(&mut self, at: Duration) {
        self.movie.seek(at);
    }

    pub(crate) fn play(&mut self) {
        self.movie.play();
    }

    pub(crate) fn pause(&mut self) {
        self.movie.pause();
    }
}
