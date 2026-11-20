use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rikka_ring::{Consumer, Producer};
use rikka_worker::{EventFd, EventNotifier, event_fd};

const NONE: u64 = u64::MAX;

#[derive(Debug)]
pub(crate) struct Ring {
    frame: usize,
    capacity: usize,
    discard: AtomicU64,
    cut: AtomicU64,
    resume: AtomicU64,
    latency: AtomicU64,
    room: EventNotifier,
    filled: EventNotifier,
}

#[derive(Debug)]
pub(crate) struct RingWriter {
    producer: Producer<u8>,
    ring: Arc<Ring>,
}

#[derive(Debug)]
pub(crate) struct RingReader {
    consumer: Consumer<u8>,
    ring: Arc<Ring>,
}

pub(crate) struct Opened {
    pub(crate) writer: RingWriter,
    pub(crate) reader: RingReader,
    pub(crate) filled: EventFd,
}

pub(crate) fn open(frames: usize, frame: usize, room: EventNotifier) -> std::io::Result<Opened> {
    let (filled, woken) = event_fd()?;
    let frame = frame.max(1);
    let capacity = frames.max(1) * frame;
    let (producer, consumer) = rikka_ring::channel(capacity);
    let ring = Arc::new(Ring {
        frame,
        capacity,
        discard: AtomicU64::new(0),
        cut: AtomicU64::new(NONE),
        resume: AtomicU64::new(0),
        latency: AtomicU64::new(0),
        room,
        filled,
    });
    Ok(Opened {
        writer: RingWriter {
            producer,
            ring: Arc::clone(&ring),
        },
        reader: RingReader { consumer, ring },
        filled: woken,
    })
}

impl Ring {
    pub(crate) fn frame(&self) -> usize {
        self.frame
    }

    pub(crate) fn discards(&self) -> u64 {
        self.discard.load(Ordering::Acquire)
    }

    pub(crate) fn wake_consumer(&self) {
        if let Err(error) = self.filled.notify() {
            tracing::debug!("failed to wake ALSA writer: {error}");
        }
    }
}

impl RingWriter {
    pub(crate) fn frame(&self) -> usize {
        self.ring.frame
    }

    pub(crate) fn written(&self) -> u64 {
        self.producer.position()
    }

    pub(crate) fn free(&self) -> usize {
        let room = self.ring.capacity.saturating_sub(self.producer.len());
        room / self.ring.frame * self.ring.frame
    }

    pub(crate) fn push(&mut self, data: &[u8]) -> usize {
        let frame = self.ring.frame;
        let count = self.free().min(data.len() / frame * frame);
        if count == 0 {
            return 0;
        }
        let pushed = self.producer.push_slice(&data[..count]);
        self.ring.wake_consumer();
        pushed
    }

    pub(crate) fn discard(&self) {
        self.ring.cut.store(NONE, Ordering::Release);
        self.ring.discard.store(self.written(), Ordering::Release);
        self.ring.wake_consumer();
    }

    pub(crate) fn splice(&self, at: u64) {
        self.ring.resume.store(self.written(), Ordering::Release);
        self.ring.cut.fetch_min(at, Ordering::AcqRel);
        self.ring.wake_consumer();
    }

    pub(crate) fn pulled(&self) -> u64 {
        self.read().max(self.ring.discards())
    }

    pub(crate) fn played_frames(&self) -> u64 {
        let read = self.read() / self.ring.frame as u64;
        read.saturating_sub(self.ring.latency.load(Ordering::Acquire))
    }

    fn read(&self) -> u64 {
        self.written() - self.producer.len() as u64
    }
}

impl RingReader {
    pub(crate) fn ring(&self) -> &Arc<Ring> {
        &self.ring
    }

    pub(crate) fn pull(&mut self, into: &mut [u8]) -> usize {
        let ring = &*self.ring;
        self.consumer.skip_to(ring.discards());
        let cut = ring.cut.load(Ordering::Acquire);
        if cut != NONE && self.consumer.position() >= cut {
            let resume = ring.resume.load(Ordering::Acquire);
            if ring
                .cut
                .compare_exchange(cut, NONE, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.consumer.skip_to(resume);
            }
        }
        let cut = ring.cut.load(Ordering::Acquire);
        let end = if cut != NONE && self.consumer.position() < cut {
            cut
        } else {
            NONE
        };
        let whole = into.len() / ring.frame * ring.frame;
        let count = self.consumer.pop_slice_before(&mut into[..whole], end);
        if self.consumer.len() * 4 < ring.capacity * 3
            && let Err(error) = ring.room.notify()
        {
            tracing::debug!("failed to wake hifi engine: {error}");
        }
        count
    }

    pub(crate) fn set_latency(&self, frames: u64) {
        self.ring.latency.store(frames, Ordering::Release);
    }
}
