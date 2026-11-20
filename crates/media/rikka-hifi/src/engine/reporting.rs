use std::time::Instant;

use rikka_hifi_core::SignalPath;

use crate::engine::backend::Backend;
use crate::engine::timeline::Transition;
use crate::engine::{Engine, State, TICK};
use crate::event::{Event, OutputState, Refusal};
use crate::notice::{Notice, OutputNotice};

impl Engine {
    pub(super) fn report(&mut self) {
        let Some(backend) = &self.backend else {
            return;
        };
        let played = backend.ring().played_frames();
        if let Some(transition) = self.timeline.cross(played) {
            match transition {
                Transition::Started(source) => self.emit(Event::Started(source)),
                Transition::Advanced { from, to } => {
                    if self
                        .previous
                        .as_ref()
                        .is_some_and(|previous| previous.id() == from)
                    {
                        self.previous = None;
                    }
                    self.emit(Event::Advanced { from, to });
                }
            }
            self.report_path();
        }
        let drained = self.timeline.end().is_some_and(|end| played >= end);
        if self.waiting && drained {
            self.advance_after_drain();
            return;
        }
        let finished = self
            .decoding
            .as_ref()
            .is_some_and(|playing| playing.finished);
        if drained && finished && self.next.is_none() {
            let ended = self.decoding.as_ref().map(|playing| playing.id());
            self.idle();
            if let Some(source) = ended {
                self.emit(Event::Ended(source));
            }
            return;
        }
        if self.state == State::Playing && self.told_position.elapsed() >= TICK.mul_f32(0.9) {
            self.told_position = Instant::now();
            if let Some((source, at)) = self.timeline.locate(played) {
                self.emit(Event::Position { source, at });
            }
        }
    }

    pub(super) fn report_path(&mut self) {
        let Some(backend) = &self.backend else {
            return;
        };
        let sounding = match &self.previous {
            Some(previous) if self.timeline.current_source() == Some(previous.id()) => {
                Some(previous)
            }
            _ => self.decoding.as_ref(),
        };
        let Some(playing) = sounding else {
            return;
        };
        let mut stages = playing.pipeline.stages();
        let output = backend.output_path(self.volume, &mut stages);
        let path = SignalPath {
            source: playing.stream.format,
            stages,
            output,
        };
        if self.told_path.as_ref() == Some(&path) {
            return;
        }
        self.told_path = Some(path.clone());
        self.emit(Event::Path(Box::new(path)));
    }

    pub(super) fn handle_notice(&mut self, output_notice: OutputNotice) {
        let current = output_notice.output == self.outputs && self.backend.is_some();
        match output_notice.notice {
            Notice::ReleaseRequested(released) if released.is_canceled() => {}
            Notice::ReleaseRequested(released) => {
                self.fall_back(Refusal::Released);
                let _ = released.send(());
            }
            _ if !current => {}
            Notice::Graph(path) => {
                if let Some(Backend::Shared { graph, .. }) = &mut self.backend {
                    *graph = Some(path);
                    self.report_path();
                }
            }
            Notice::SharedFailed(message) => {
                tracing::warn!("shared output failed, reopening: {message}");
                self.relocate();
            }
            Notice::Lost(message) => {
                tracing::info!("exclusive output lost: {message}");
                self.fall_back(Refusal::Lost);
            }
        }
    }

    fn fall_back(&mut self, reason: Refusal) {
        let Some(OutputState::Exclusive(device)) = self.backend.as_ref().map(Backend::state) else {
            return;
        };
        self.exclusive_allowed = false;
        self.report_output(OutputState::FellBack { device, reason });
        if self.decoding.is_some() {
            self.relocate();
        } else {
            self.backend = None;
        }
    }
}
