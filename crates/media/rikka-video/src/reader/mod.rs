mod cache;
mod serve;
mod shared;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use self::cache::Window;
use self::shared::Shared;
use crate::error::Result;
use crate::read::VideoInfo;
use rikka_video_core::init;
use rikka_video_core::reaper::{self, Reaped};
use rikka_worker::Stop;

const SLACK: Duration = Duration::from_micros(100);

#[derive(Clone, Copy)]
pub struct DecodedFrame<'a> {
    pub pts: Duration,
    pub duration: Duration,
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

impl std::fmt::Debug for DecodedFrame<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DecodedFrame")
            .field("pts", &self.pts)
            .field("duration", &self.duration)
            .field("width", &self.width)
            .field("height", &self.height)
            .field("rgba", &self.rgba.len())
            .finish()
    }
}

pub struct FrameReader<T: Clone + Send + 'static> {
    shared: Arc<Shared<T>>,
    _thread: Reaped<Closer<T>>,
}

impl<T: Clone + Send + 'static> FrameReader<T> {
    pub fn open(
        path: impl AsRef<Path>,
        convert: impl FnMut(DecodedFrame<'_>) -> Option<T> + Send + 'static,
    ) -> Result<Self> {
        init::ensure()?;
        let path = path.as_ref().to_path_buf();
        let shared = Arc::new(Shared::new());
        let serving = Arc::clone(&shared);
        let thread = reaper::spawn(
            "rikka-video-reader",
            Closer(Arc::clone(&shared)),
            move || {
                serve::serve(serving, &path, Box::new(convert));
            },
        )?;
        Ok(Self {
            shared,
            _thread: thread,
        })
    }

    pub fn info(&self) -> Option<VideoInfo> {
        self.shared.lock().info
    }

    pub fn failure(&self) -> Option<String> {
        self.shared.lock().failure.clone()
    }

    pub fn want(&self, at: Duration, ahead: Duration) {
        let start = at.saturating_add(SLACK);
        let window = Window {
            start,
            end: start.saturating_add(ahead),
        };
        self.shared.change(|state| {
            let changed = state.window != Some(window);
            state.window = Some(window);
            changed
        });
    }

    pub fn frame(&self, at: Duration) -> Option<(Duration, T)> {
        let state = self.shared.lock();
        let entry = state.cache.covering(at.saturating_add(SLACK))?;
        entry.value.clone().map(|value| (entry.pts, value))
    }

    pub fn fit(&self, size: Option<(u32, u32)>) {
        self.shared.change(|state| {
            let changed = state.bounds != size;
            if changed {
                state.bounds = size;
                state.fit += 1;
            }
            changed
        });
    }

    pub fn on_ready(&self, wake: impl Fn() + Send + Sync + 'static) {
        self.shared.lock().ready = Some(Arc::new(wake));
    }
}

impl<T: Clone + Send + 'static> std::fmt::Debug for FrameReader<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.shared.lock();
        formatter
            .debug_struct("FrameReader")
            .field("info", &state.info)
            .field("failure", &state.failure)
            .finish_non_exhaustive()
    }
}

struct Closer<T>(Arc<Shared<T>>);

impl<T> Stop for Closer<T> {
    fn stop(&self) {
        self.0.change(|state| {
            state.closed = true;
            state.ready = None;
            true
        });
    }
}
