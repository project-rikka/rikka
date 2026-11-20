use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;
use std::time::Instant;
use std::{fmt, io};

use rikka_compositor_reactor::Reactor;
use rikka_renderer::FrameTimeline;

use crate::context::capture_ctx;
use crate::state::Compositor;

pub(crate) struct TimelineWatch {
    asks: Option<mpsc::Sender<u64>>,
    latest_frame: u64,
    thread: Option<JoinHandle<()>>,
}

impl TimelineWatch {
    pub(crate) fn start(
        reactor: &Reactor<Compositor>,
        timeline: Arc<FrameTimeline>,
    ) -> io::Result<Self> {
        let reached = reactor.mailbox(
            |compositor: &mut Compositor, (finished, at): (u64, Instant)| {
                capture_ctx!(compositor).gpu_reached(finished, at);
            },
        )?;
        let (asks, waits) = mpsc::channel::<u64>();
        let thread = std::thread::Builder::new()
            .name("rikka-gpu-timeline".to_owned())
            .spawn(move || {
                for value in waits {
                    if let Err(error) = timeline.wait(value) {
                        tracing::warn!("cannot wait on renderer timeline: {error}");
                        break;
                    }
                    if reached.send((value, Instant::now())).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            asks: Some(asks),
            latest_frame: 0,
            thread: Some(thread),
        })
    }

    pub(crate) fn ask(&mut self, frame: u64) {
        if frame <= self.latest_frame {
            return;
        }
        self.latest_frame = frame;
        if let Some(asks) = &self.asks
            && asks.send(frame).is_err()
        {
            tracing::warn!("renderer timeline watcher stopped");
        }
    }
}

impl Drop for TimelineWatch {
    fn drop(&mut self) {
        self.asks = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl fmt::Debug for TimelineWatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TimelineWatch")
            .field("latest_frame", &self.latest_frame)
            .finish_non_exhaustive()
    }
}
