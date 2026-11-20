use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use rikka_ring::Producer;
use rikka_worker::EventNotifier;

pub(crate) struct Intake {
    producer: Producer<f32>,
    channels: usize,
    dropped: Arc<AtomicU64>,
    arrived: EventNotifier,
}

impl Intake {
    pub(crate) fn new(
        producer: Producer<f32>,
        channels: u16,
        dropped: Arc<AtomicU64>,
        arrived: EventNotifier,
    ) -> Intake {
        Intake {
            producer,
            channels: usize::from(channels),
            dropped,
            arrived,
        }
    }

    #[cfg(unix)]
    pub(crate) fn channels(&self) -> usize {
        self.channels
    }

    pub(crate) fn push(&mut self, samples: &[f32]) {
        let room = self.producer.free();
        let kept = samples.len().min(room - room % self.channels);
        self.producer.push_slice(&samples[..kept]);
        if kept < samples.len() {
            self.dropped
                .fetch_add((samples.len() - kept) as u64, Ordering::Relaxed);
        }
    }

    pub(crate) fn ring(&self) {
        ring(&self.arrived);
    }

    #[cfg(unix)]
    pub(crate) fn bell(&self) -> EventNotifier {
        self.arrived.clone()
    }
}

pub(crate) fn ring(bell: &EventNotifier) {
    if let Err(error) = bell.notify() {
        tracing::debug!("failed to signal captured sound: {error}");
    }
}
