use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rikka_capture::{Capturer, Screen, Source};

use crate::frame::Frame;
use crate::shot::CURSOR;
use crate::watcher::{Event, Watcher};

const FRAME_INTERVAL: Duration = Duration::from_millis(16);

const CAPTURE_TIMEOUT: Duration = Duration::from_millis(500);

const MAX_CONSECUTIVE_FAILURES: u32 = 8;

pub(crate) struct FrameStream {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FrameStream {
    pub(crate) fn start(socket: PathBuf, watcher: Watcher) -> FrameStream {
        let stop = Arc::new(AtomicBool::new(false));
        let thread = std::thread::Builder::new()
            .name("rikka-computer-capture".into())
            .spawn({
                let stop = Arc::clone(&stop);
                let watcher = watcher.clone();
                move || stream(&socket, &watcher, &stop)
            });

        let thread = match thread {
            Ok(thread) => Some(thread),
            Err(error) => {
                tracing::warn!(%error, "failed to spawn capture thread");
                watcher.notify(Event::Disconnected);
                None
            }
        };
        FrameStream { stop, thread }
    }
}

impl Drop for FrameStream {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take()
            && thread.join().is_err()
        {
            tracing::warn!("capture thread panicked");
        }
    }
}

impl std::fmt::Debug for FrameStream {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FrameStream")
            .finish_non_exhaustive()
    }
}

fn stream(socket: &Path, watcher: &Watcher, stop: &AtomicBool) {
    let mut capturer = match open(socket) {
        Ok(capturer) => capturer,
        Err(error) => {
            tracing::warn!(%error, "failed to open screen capture");
            watcher.notify(Event::Disconnected);
            return;
        }
    };

    let mut pixels = Vec::new();
    let mut failures = 0;

    while !stop.load(Ordering::Relaxed) {
        let started = Instant::now();
        match capturer.capture_before(&mut pixels, Some(started + CAPTURE_TIMEOUT)) {
            Ok(None) => continue,
            Ok(Some((width, height))) => {
                failures = 0;
                let frame = Frame::new(width, height, std::mem::take(&mut pixels));
                watcher.notify(Event::Frame(frame));
            }
            Err(error) => {
                failures += 1;
                if failures >= MAX_CONSECUTIVE_FAILURES {
                    tracing::debug!(%error, "screen capture keeps failing, stopping stream");
                    watcher.notify(Event::Disconnected);
                    return;
                }
            }
        }

        if let Some(remaining) = FRAME_INTERVAL.checked_sub(started.elapsed()) {
            std::thread::sleep(remaining);
        }
    }
}

fn open(socket: &Path) -> rikka_capture::Result<Capturer> {
    let screen = Screen::connect_to(socket)?;
    Capturer::open(screen, &Source::FirstOutput, CURSOR)
}
