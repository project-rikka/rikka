use std::time::Instant;

use futures_util::{Stream, StreamExt};
use rikka_seam::Request;
use rikka_worker::Observable;

#[derive(Clone, Debug)]
pub struct ScreenRecording {
    since: Observable<Option<Instant>>,
    stop: Request,
}

impl ScreenRecording {
    #[must_use]
    pub fn new(stop: Request) -> ScreenRecording {
        ScreenRecording {
            since: Observable::default(),
            stop,
        }
    }

    #[must_use]
    pub fn since(&self) -> Option<Instant> {
        self.since.get()
    }

    #[must_use]
    pub fn stop(&self) -> Request {
        self.stop.clone()
    }

    pub fn began(&self) {
        self.since.modify(|since| {
            let began = since.is_none();
            if began {
                *since = Some(Instant::now());
            }
            began
        });
    }

    pub fn ended(&self) {
        self.since.modify(|since| since.take().is_some());
    }

    pub fn changes(&self) -> impl Stream<Item = bool> + Send + 'static {
        self.since.changes().map(|since| since.is_some())
    }
}
