use rikka_sync::{Condvar, Mutex, MutexGuard};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use rikka_worker::{EventNotifier, Stop};

pub(crate) struct Feed<T> {
    queue: Mutex<Queue<T>>,
    changed: Condvar,
    capacity: usize,
    frame: usize,
    seekable: bool,
}

struct Queue<T> {
    items: VecDeque<T>,
    finished: bool,
    closed: bool,
    seeking: Option<Duration>,
    seek_taken: bool,
    restart: Option<Duration>,
    failure: Option<rikka_audio_decode::Error>,
    wake: Option<EventNotifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stopped {
    Closed,
    Seeking,
}

#[derive(Debug)]
pub(crate) enum ReadStatus {
    Some,
    Waiting,
    Ended,
    Restarted(Duration),
    Failed(rikka_audio_decode::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Idle {
    Closed,
    Seek,
}

impl<T: Copy> Feed<T> {
    pub(crate) fn new(capacity: usize, frame: usize, seekable: bool) -> Arc<Feed<T>> {
        let frame = frame.max(1);
        Arc::new(Feed {
            queue: Mutex::new(Queue {
                items: VecDeque::with_capacity(capacity),
                finished: false,
                closed: false,
                seeking: None,
                seek_taken: false,
                restart: None,
                failure: None,
                wake: None,
            }),
            changed: Condvar::new(),
            capacity: capacity.max(frame),
            frame,
            seekable,
        })
    }

    pub(crate) fn push(&self, data: &[T]) -> Result<(), Stopped> {
        let mut rest = data;
        let mut queue = self.lock();
        loop {
            if queue.closed {
                return Err(Stopped::Closed);
            }
            if queue.seeking.is_some() {
                return Err(Stopped::Seeking);
            }
            let fitting = (self.capacity - queue.items.len()).min(rest.len());
            if fitting > 0 {
                queue.items.extend(&rest[..fitting]);
                rest = &rest[fitting..];
                queue.ring();
            }
            if rest.is_empty() {
                return Ok(());
            }
            queue = self.changed.wait(queue);
        }
    }

    pub(crate) fn restart(&self, at: Duration) {
        let mut queue = self.lock();
        queue.items.clear();
        queue.finished = false;
        queue.seeking = None;
        queue.restart = Some(at);
        queue.ring();
    }

    pub(crate) fn finish(&self) {
        let mut queue = self.lock();
        queue.finished = true;
        queue.ring();
    }

    pub(crate) fn fail(&self, error: rikka_audio_decode::Error) {
        let mut queue = self.lock();
        queue.failure = Some(error);
        queue.finished = true;
        queue.seeking = None;
        queue.ring();
    }

    pub(crate) fn take_seek_request(&self) -> Option<Duration> {
        let mut queue = self.lock();
        if queue.seek_taken {
            return None;
        }
        let target = queue.seeking?;
        queue.seek_taken = true;
        Some(target)
    }

    pub(crate) fn idle(&self) -> Idle {
        let mut queue = self.lock();
        loop {
            if queue.closed {
                return Idle::Closed;
            }
            if queue.seeking.is_some() && !queue.seek_taken {
                return Idle::Seek;
            }
            queue = self.changed.wait(queue);
        }
    }

    pub(crate) fn close(&self) {
        let mut queue = self.lock();
        queue.closed = true;
        queue.items.clear();
        drop(queue);
        self.changed.notify_all();
    }

    pub(crate) fn attach(&self, wake: EventNotifier) {
        let mut queue = self.lock();
        queue.wake = Some(wake);
    }

    pub(crate) fn is_seekable(&self) -> bool {
        self.seekable
    }

    pub(crate) fn seek(&self, to: Duration) {
        let mut queue = self.lock();
        queue.items.clear();
        queue.finished = false;
        queue.failure = None;
        queue.restart = None;
        queue.seeking = Some(to);
        queue.seek_taken = false;
        drop(queue);
        self.changed.notify_all();
    }

    pub(crate) fn take(&self, into: &mut Vec<T>, most: usize) -> ReadStatus {
        let mut queue = self.lock();
        if let Some(at) = queue.restart.take() {
            return ReadStatus::Restarted(at);
        }
        if queue.seeking.is_some() {
            return ReadStatus::Waiting;
        }
        let whole = queue.items.len() / self.frame * self.frame;
        let count = whole.min(most / self.frame * self.frame);
        if count == 0 {
            if let Some(failure) = queue.failure.take() {
                return ReadStatus::Failed(failure);
            }
            return if queue.finished {
                ReadStatus::Ended
            } else {
                ReadStatus::Waiting
            };
        }
        into.extend(queue.items.drain(..count));
        drop(queue);
        self.changed.notify_all();
        ReadStatus::Some
    }

    fn lock(&self) -> MutexGuard<'_, Queue<T>> {
        self.queue.lock()
    }
}

impl<T> Queue<T> {
    fn ring(&self) {
        if let Some(wake) = &self.wake
            && let Err(error) = wake.notify()
        {
            tracing::debug!("failed to wake hifi engine: {error}");
        }
    }
}

pub(crate) trait Close: Send + Sync {
    fn close(&self);
}

impl<T: Copy + Send> Close for Feed<T> {
    fn close(&self) {
        Feed::close(self);
    }
}

pub(crate) struct Closing(pub(crate) Arc<dyn Close>);

impl Stop for Closing {
    fn stop(&self) {
        self.0.close();
    }
}
