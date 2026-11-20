use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use super::DecodedFrame;
use super::cache::Entry;
use super::shared::Shared;
use crate::error::{Error, Result};
use crate::source::Source;
use rikka_ffmpeg::Frame;
use rikka_video_core::sw::SwConverter;

const FORWARD_REACH: Duration = Duration::from_secs(1);
const SEEK_GAIN: Duration = Duration::from_millis(500);
const FIRST_BACKOFF: Duration = Duration::from_secs(1);

pub(super) type Convert<T> = Box<dyn FnMut(DecodedFrame<'_>) -> Option<T> + Send>;

pub(super) fn serve<T: Clone + Send + 'static>(
    shared: Arc<Shared<T>>,
    path: &Path,
    convert: Convert<T>,
) {
    let outcome = Source::open(path).and_then(|(source, info)| {
        shared.change(|state| {
            state.info = Some(info);
            true
        });
        Server::new(Arc::clone(&shared), source, convert)?.run()
    });
    let cached = {
        let mut state = shared.lock();
        if let Err(error) = outcome {
            tracing::warn!("failed to read {}: {error}", path.display());
            state.failure = Some(error.to_string());
        }
        state.cache.take()
    };
    drop(cached);
    if let Some(wake) = shared.lock().ready.clone() {
        wake();
    }
}

struct Server<T> {
    shared: Arc<Shared<T>>,
    convert: Convert<T>,
    source: Source,
    converter: SwConverter,
    rgba: Vec<u8>,
    current: Frame,
    scratch: Frame,
    run: Option<Run>,
}

struct Run {
    target: Duration,
    backoff: Duration,
    opens: bool,
    started: bool,
    ended: bool,
    reached: Duration,
    pending: Option<Pending>,
}

#[derive(Clone, Copy)]
struct Pending {
    from: Duration,
    pts: Duration,
}

enum Action {
    Seek(Duration),
    Decode,
    Stop,
}

impl<T: Clone + Send + 'static> Server<T> {
    fn new(shared: Arc<Shared<T>>, source: Source, convert: Convert<T>) -> Result<Self> {
        Ok(Self {
            shared,
            convert,
            source,
            converter: SwConverter::new(),
            rgba: Vec::new(),
            current: Frame::new()?,
            scratch: Frame::new()?,
            run: None,
        })
    }

    fn run(mut self) -> Result<()> {
        loop {
            match self.next_action() {
                Action::Stop => return Ok(()),
                Action::Seek(to) => self.seek(to, to, FIRST_BACKOFF)?,
                Action::Decode => self.decode()?,
            }
        }
    }

    fn next_action(&self) -> Action {
        let mut state = self.shared.lock();
        loop {
            if state.closed {
                return Action::Stop;
            }
            if let Some(window) = state.window {
                let evicted = state.cache.evict(window);
                if !evicted.is_empty() {
                    drop(state);
                    drop(evicted);
                    state = self.shared.lock();
                    continue;
                }
            }
            if let Some(missing) = state.missing() {
                return self.decide(missing);
            }
            state = self.shared.wait(state);
        }
    }

    fn decide(&self, missing: Duration) -> Action {
        let Some(run) = &self.run else {
            return Action::Seek(missing);
        };
        if run.ended || missing < run.reached {
            return Action::Seek(missing);
        }
        if missing <= run.target.saturating_add(FORWARD_REACH) {
            return Action::Decode;
        }
        let from = run.pending.map_or(run.reached, |pending| pending.pts);
        let forward = match self.source.keyframe_before(missing) {
            Some(keyframe) if keyframe > from.saturating_add(SEEK_GAIN) => false,
            Some(_) if self.source.indexed_beyond(missing) => true,
            _ => missing <= from.saturating_add(FORWARD_REACH),
        };
        match forward {
            true => Action::Decode,
            false => Action::Seek(missing),
        }
    }

    fn seek(&mut self, target: Duration, to: Duration, backoff: Duration) -> Result<()> {
        let opens = !self.source.seek(to)? || to.is_zero();
        self.current.unref();
        self.run = Some(Run {
            target,
            backoff,
            opens,
            started: false,
            ended: false,
            reached: match opens {
                true => Duration::ZERO,
                false => target,
            },
            pending: None,
        });
        Ok(())
    }

    fn decode(&mut self) -> Result<()> {
        let Some(mut run) = self.run.take() else {
            return Ok(());
        };
        let decoded = self.source.next(&mut self.scratch)?;
        let first = !run.started;
        run.started = true;
        match decoded {
            Some(pts) if first && !run.opens && pts > run.target => {
                return self.seek_earlier(&run);
            }
            Some(pts) => {
                if let Some(pending) = run.pending {
                    self.settle(pending, pts.max(pending.pts), false)?;
                }
                std::mem::swap(&mut self.current, &mut self.scratch);
                let from = match first && run.opens {
                    true => Duration::ZERO,
                    false => pts,
                };
                run.pending = Some(Pending { from, pts });
                run.reached = from;
            }
            None if first && !run.opens => return self.seek_earlier(&run),
            None if first => {
                return Err(Error::Unsupported("video has no frames".into()));
            }
            None => {
                if let Some(pending) = run.pending.take() {
                    self.settle(pending, Duration::MAX, true)?;
                }
                run.ended = true;
            }
        }
        self.run = Some(run);
        Ok(())
    }

    fn seek_earlier(&mut self, run: &Run) -> Result<()> {
        let to = run.target.saturating_sub(run.backoff);
        self.seek(run.target, to, run.backoff * 2)
    }

    fn settle(&mut self, pending: Pending, end: Duration, last: bool) -> Result<()> {
        let (bounds, fit) = {
            let state = self.shared.lock();
            let visible = state
                .window
                .is_some_and(|window| window.overlaps(pending.from, end));
            if !visible || state.cache.holds(pending.from, state.fit) {
                drop(state);
                self.current.unref();
                return Ok(());
            }
            (state.bounds, state.fit)
        };

        let converted = self
            .converter
            .rgba_within(&self.current, bounds, &mut self.rgba);
        let duration = match last {
            true => self.source.length(&self.current),
            false => end - pending.pts,
        };
        self.current.unref();
        let (width, height) = converted?;
        let value = (self.convert)(DecodedFrame {
            pts: pending.pts,
            duration,
            width,
            height,
            rgba: &self.rgba,
        });

        let (replaced, wake) = {
            let mut state = self.shared.lock();
            let replaced = state.cache.insert(
                pending.from,
                Entry {
                    pts: pending.pts,
                    end,
                    value,
                    fit,
                },
            );
            (replaced, state.ready.clone())
        };
        drop(replaced);
        if let Some(wake) = wake {
            wake();
        }
        Ok(())
    }
}
