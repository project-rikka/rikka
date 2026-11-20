use std::time::Duration;

use crate::engine::playing::Playing;
use crate::engine::{Engine, State};
use crate::event::{Event, Failure};
use crate::ring::RingWriter;
use crate::source::ReadStatus;

const CHUNK_FRAMES: usize = 4096;

impl Engine {
    pub(super) fn feed(&mut self) {
        loop {
            let Some(backend) = self.backend.as_mut() else {
                return;
            };
            let ring = backend.ring_mut();
            if !self.pending.is_empty() {
                let pushed = ring.push(&self.pending);
                self.pending.drain(..pushed);
                if !self.pending.is_empty() {
                    return;
                }
            }
            if ring.free() == 0 {
                return;
            }
            let Some(playing) = self.decoding.as_mut() else {
                return;
            };
            if playing.finished {
                if !self.chain() {
                    return;
                }
                continue;
            }
            self.pcm.clear();
            self.dsd.clear();
            match playing
                .stream
                .take(&mut self.pcm, &mut self.dsd, CHUNK_FRAMES)
            {
                ReadStatus::Some if playing.stream.format.is_dsd() => {
                    playing.pipeline.dsd(&self.dsd, &mut self.pending);
                }
                ReadStatus::Some => playing.pipeline.pcm(&self.pcm, &mut self.pending),
                ReadStatus::Waiting => return,
                ReadStatus::Ended => self.finish_decoding(),
                ReadStatus::Restarted(at) => self.restarted(at),
                ReadStatus::Failed(error) => {
                    let source = Some(playing.id());
                    self.emit(Event::Failed {
                        source,
                        failure: Failure::Unreadable(error.to_string()),
                    });
                    self.finish_decoding();
                }
            }
        }
    }

    pub(super) fn cursor(&self, ring: &RingWriter) -> u64 {
        (ring.written() + self.pending.len() as u64) / ring.frame() as u64
    }

    fn written(&self) -> u64 {
        self.backend
            .as_ref()
            .map_or(0, |backend| self.cursor(backend.ring()))
    }

    fn finish_decoding(&mut self) {
        let Some(playing) = self.decoding.as_mut() else {
            return;
        };
        playing.pipeline.finish(&mut self.pending);
        playing.finished = true;
        let id = playing.id();
        let end = self.written();
        self.timeline.finish(id, end);
    }

    fn restarted(&mut self, at: Duration) {
        if let Some(backend) = &self.backend {
            backend.ring().discard();
        }
        self.pending.clear();
        let Some(playing) = self.decoding.as_mut() else {
            return;
        };
        playing.pipeline.reset();
        playing.finished = false;
        let (id, rate) = (playing.id(), playing.plan().rate);
        let cursor = self.written();
        self.timeline.restart(id, cursor, at, rate);
    }

    fn chain(&mut self) -> bool {
        if self.waiting {
            return false;
        }
        let Some(next) = self.next.take() else {
            return false;
        };
        let Some(plan) = self.backend.as_ref().map(super::backend::Backend::plan) else {
            self.next = Some(next);
            return false;
        };
        if self.settled_plan(&next.format) != Some(plan) {
            self.next = Some(next);
            self.waiting = true;
            return false;
        }
        let Some(backend) = &self.backend else {
            return false;
        };
        let volume = backend.software_volume(self.volume);
        let cursor = self.cursor(backend.ring());
        self.timeline
            .begin(next.id, cursor, Duration::ZERO, plan.rate);
        let playing = Playing::new(next, plan, volume, &self.replay_gain);
        self.previous = self.decoding.replace(playing);
        true
    }

    pub(super) fn withdraw_chained(&mut self) {
        let (Some(backend), Some(successor)) = (&self.backend, &self.decoding) else {
            return;
        };
        if self.previous.is_none() || self.timeline.current_source() == Some(successor.id()) {
            return;
        }
        let Some(boundary) = self.timeline.start_of(successor.id()) else {
            return;
        };
        let ring = backend.ring();
        let seam = boundary * ring.frame() as u64;
        if ring.pulled() > seam {
            return;
        }
        ring.splice(seam);
        self.pending.clear();
        self.timeline.cancel_from(boundary);
        self.decoding = self.previous.take();
    }

    pub(super) fn advance_after_drain(&mut self) {
        self.waiting = false;
        let Some(next) = self.next.take() else {
            return;
        };
        self.previous = None;
        self.decoding = None;
        self.pending.clear();
        let claim = self
            .backend
            .take()
            .and_then(super::backend::Backend::into_claim);
        let Some(plan) = self.output_for(&next.format, claim) else {
            self.idle();
            return;
        };
        let Some(backend) = &self.backend else {
            return;
        };
        let volume = backend.software_volume(self.volume);
        let cursor = self.cursor(backend.ring());
        backend.set_running(self.state == State::Playing);
        self.timeline
            .restart(next.id, cursor, Duration::ZERO, plan.rate);
        self.decoding = Some(Playing::new(next, plan, volume, &self.replay_gain));
        self.report_path();
    }
}
