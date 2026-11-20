use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};

use discord_core::{EncodedVideo, VideoShape};
use rikka_sync::Mutex;
use tokio::sync::Notify;

use crate::format::{SILENCE, SILENCE_FRAMES};

const FRAMES: usize = 16;

const VIDEO_FRAMES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Outgoing {
    Frame(Vec<u8>),
    Speaking(bool),
    Video(EncodedVideo),
    Shape(Option<VideoShape>),
    Refresh(u32),
}

#[derive(Debug, Default)]
pub(crate) struct Outbox {
    queue: Mutex<VecDeque<Outgoing>>,
    wake: Notify,
    closed: AtomicBool,
    overflowed: AtomicBool,
}

impl Outbox {
    pub(crate) fn frame(&self, opus: &[u8]) {
        if self.is_closed() {
            return;
        }
        push_frame(&mut self.queue.lock(), opus.to_vec());
        self.wake.notify_one();
    }

    pub(crate) fn speaking(&self, on: bool) {
        if self.is_closed() {
            return;
        }
        let mut queue = self.queue.lock();
        queue.push_back(Outgoing::Speaking(on));
        if !on {
            for _ in 0..SILENCE_FRAMES {
                push_frame(&mut queue, SILENCE.to_vec());
            }
        }
        drop(queue);
        self.wake.notify_one();
    }

    pub(crate) fn video(&self, frame: EncodedVideo) {
        if self.is_closed() {
            return;
        }
        let mut queue = self.queue.lock();
        let is_video = |item: &Outgoing| matches!(item, Outgoing::Video(_));
        if queue.iter().filter(|item| is_video(item)).count() >= VIDEO_FRAMES {
            queue.retain(|item| !is_video(item));
            self.overflowed.store(true, Ordering::Release);
        }
        queue.push_back(Outgoing::Video(frame));
        drop(queue);
        self.wake.notify_one();
    }

    pub(crate) fn shape(&self, shape: Option<VideoShape>) {
        if self.is_closed() {
            return;
        }
        self.queue.lock().push_back(Outgoing::Shape(shape));
        self.wake.notify_one();
    }

    pub(crate) fn refresh(&self, source: u32) {
        if self.is_closed() {
            return;
        }
        self.queue.lock().push_back(Outgoing::Refresh(source));
        self.wake.notify_one();
    }

    pub(crate) fn overflowed(&self) -> bool {
        self.overflowed.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn close(&self) -> bool {
        let first = !self.closed.swap(true, Ordering::AcqRel);
        if first {
            self.queue.lock().clear();
            self.wake.notify_one();
        }
        first
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub(crate) fn take(&self) -> VecDeque<Outgoing> {
        std::mem::take(&mut *self.queue.lock())
    }

    pub(crate) async fn notified(&self) {
        self.wake.notified().await;
    }
}

fn push_frame(queue: &mut VecDeque<Outgoing>, frame: Vec<u8>) {
    let is_frame = |item: &Outgoing| matches!(item, Outgoing::Frame(_));
    if queue.iter().filter(|item| is_frame(item)).count() >= FRAMES
        && let Some(oldest) = queue.iter().position(is_frame)
    {
        queue.remove(oldest);
    }
    queue.push_back(Outgoing::Frame(frame));
}
