use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use std::time::Duration;

use crate::stream::StreamShared;
use crate::suspend::OutputWaker;
use crate::time::{duration_to_frames, frames_to_duration};
use crate::voice::{MAX_SPEED, MIN_SPEED, VoiceShared};
use crate::watch::{PlaybackEvent, PlaybackWatch, Watched};

#[derive(Debug, Clone)]
pub struct PlaybackHandle {
    pub(crate) shared: Arc<VoiceShared>,
    pub(crate) stream: Option<Arc<StreamShared>>,
    pub(crate) fed: bool,
    pub(crate) source_rate: u32,
    pub(crate) duration: Option<Duration>,
    pub(crate) waker: OutputWaker,
    pub(crate) watched: Weak<Watched>,
}

impl PlaybackHandle {
    pub fn set_volume(&self, volume: f32) {
        self.shared.volume.set(volume.max(0.0));
    }

    pub fn volume(&self) -> f32 {
        self.shared.volume.get()
    }

    pub fn set_speed(&self, speed: f32) {
        self.shared.speed.set(speed.clamp(MIN_SPEED, MAX_SPEED));
    }

    pub fn speed(&self) -> f32 {
        self.shared.speed.get()
    }

    pub fn set_pan(&self, pan: f32) {
        self.shared.pan.set(pan.clamp(-1.0, 1.0));
    }

    pub fn pan(&self) -> f32 {
        self.shared.pan.get()
    }

    pub fn set_looping(&self, looping: bool) {
        self.shared.looping.store(looping, Ordering::Relaxed);
        self.notify_producer();
    }

    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::Relaxed);
        self.notify_producer();
    }

    pub fn resume(&self) {
        self.shared.paused.store(false, Ordering::Relaxed);
        self.notify_producer();
        self.waker.wake();
    }

    pub fn is_paused(&self) -> bool {
        self.shared.paused.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.notify_producer();
        self.waker.wake();
    }

    pub fn watch(&self, listener: impl Fn(PlaybackEvent) + Send + Sync + 'static) -> PlaybackWatch {
        let listener: Arc<dyn Fn(PlaybackEvent) + Send + Sync> = Arc::new(listener);
        let watch = self.shared.listeners.add(Arc::clone(&listener));
        if let Some(watched) = self.watched.upgrade() {
            watched.add(self.clone());
        }
        if self.shared.listeners.has_finished() {
            listener(PlaybackEvent::Finished);
        }
        watch
    }

    pub fn is_finished(&self) -> bool {
        self.shared.finished.load(Ordering::Relaxed)
    }

    pub fn seek(&self, position: Duration) {
        match &self.stream {
            Some(stream) if self.fed => stream.request_flush(),
            Some(stream) => stream.request_seek(position.as_secs_f64()),
            None => {
                let frame = duration_to_frames(position, self.source_rate);
                self.shared.seek_frame.store(frame, Ordering::Relaxed);
            }
        }
        self.waker.wake();
    }

    pub fn flush(&self) {
        match &self.stream {
            Some(stream) if self.fed => stream.request_flush(),
            Some(stream) => stream.request_seek(stream.position().as_secs_f64()),
            None => {}
        }
        self.waker.wake();
    }

    pub fn position(&self) -> Duration {
        match &self.stream {
            Some(stream) => stream.position(),
            None => frames_to_duration(
                self.shared.position_frames.load(Ordering::Relaxed),
                self.source_rate,
            ),
        }
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    pub fn underruns(&self) -> u32 {
        self.stream.as_ref().map_or(0, |stream| stream.underruns())
    }

    fn notify_producer(&self) {
        if let Some(stream) = &self.stream {
            stream.notify_producer();
        }
    }
}
