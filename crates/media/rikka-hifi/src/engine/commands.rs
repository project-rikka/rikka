use std::time::Duration;

use rikka_hifi_core::ReplayGain;

use crate::engine::playing::Playing;
use crate::engine::{Command, Engine, State};
use crate::event::{Event, Failure};
use crate::output::Output;
use crate::source::{Source, SourceId, SourceStream};

impl Engine {
    pub(super) fn handle_command(&mut self, command: Command) -> bool {
        match command {
            Command::Subscribe(subscriber) => {
                if let Some(state) = &self.told_output {
                    subscriber.send(Event::Output(state.clone()));
                }
                if let Some(path) = &self.told_path {
                    subscriber.send(Event::Path(Box::new(path.clone())));
                }
                self.subscribers.add(subscriber);
            }
            Command::Output(output) => self.choose(output),
            Command::Play(id, source) => self.play(id, source),
            Command::Queue(id, source) => self.queue(id, source),
            Command::Unqueue => self.unqueue(),
            Command::Pause => self.pause(),
            Command::Resume => self.resume(),
            Command::Stop => {
                let playing = self.decoding.is_some();
                self.idle();
                if playing {
                    self.emit(Event::Stopped);
                }
            }
            Command::Seek(to) => self.seek(to),
            Command::Volume(level) => self.set_volume(level),
            Command::ReplayGain(replay_gain) => self.set_replay_gain(replay_gain),
            Command::Quit => return false,
        }
        true
    }

    fn open(&mut self, id: SourceId, source: Source) -> Option<SourceStream> {
        match SourceStream::open(id, source, self.notifier.wake()) {
            Ok(stream) => Some(stream),
            Err(error) => {
                self.emit(Event::Failed {
                    source: Some(id),
                    failure: Failure::Unreadable(error.to_string()),
                });
                None
            }
        }
    }

    fn play(&mut self, id: SourceId, source: Source) {
        let Some(stream) = self.open(id, source) else {
            return;
        };
        self.previous = None;
        self.decoding = None;
        self.next = None;
        self.waiting = false;
        self.pending.clear();
        self.timeline.clear();
        self.exclusive_allowed = true;
        let serving = self
            .backend
            .as_ref()
            .filter(|backend| backend.matches(&self.choice))
            .map(super::backend::Backend::plan);
        let reusable_plan = match serving {
            Some(plan) => self
                .settled_plan(&stream.format)
                .filter(|settled_plan| *settled_plan == plan),
            None => None,
        };
        let plan = match reusable_plan {
            Some(plan) => {
                if let Some(backend) = &self.backend {
                    backend.ring().discard();
                }
                plan
            }
            None => {
                let claim = self
                    .backend
                    .take()
                    .and_then(super::backend::Backend::into_claim);
                match self.output_for(&stream.format, claim) {
                    Some(plan) => plan,
                    None => {
                        self.idle();
                        return;
                    }
                }
            }
        };
        let Some(backend) = &self.backend else {
            return;
        };
        let volume = backend.software_volume(self.volume);
        let cursor = self.cursor(backend.ring());
        backend.set_running(true);
        self.timeline.begin(id, cursor, Duration::ZERO, plan.rate);
        self.decoding = Some(Playing::new(stream, plan, volume, &self.replay_gain));
        self.state = State::Playing;
        self.report_path();
    }

    fn queue(&mut self, id: SourceId, source: Source) {
        let Some(stream) = self.open(id, source) else {
            return;
        };
        self.withdraw_chained();
        self.waiting = false;
        self.next = Some(stream);
    }

    fn unqueue(&mut self) {
        self.withdraw_chained();
        self.waiting = false;
        self.next = None;
    }

    fn pause(&mut self) {
        if self.state != State::Playing {
            return;
        }
        if let Some(backend) = &self.backend {
            backend.set_running(false);
        }
        self.state = State::Paused;
        self.emit(Event::Paused);
    }

    fn resume(&mut self) {
        if self.state != State::Paused {
            return;
        }
        if let Some(backend) = &self.backend {
            backend.set_running(true);
        }
        self.state = State::Playing;
        self.emit(Event::Resumed);
    }

    fn seek(&mut self, to: Duration) {
        self.restore_played();
        let Some(playing) = self.decoding.as_mut() else {
            return;
        };
        let id = playing.id();
        if !playing.stream.is_seekable() {
            self.emit(Event::Failed {
                source: Some(id),
                failure: Failure::Unseekable,
            });
            return;
        }
        playing.pipeline.reset();
        playing.finished = false;
        playing.stream.seek(to);
        let rate = playing.plan().rate;
        self.waiting = false;
        self.pending.clear();
        let Some(backend) = &self.backend else {
            return;
        };
        backend.ring().discard();
        let cursor = self.cursor(backend.ring());
        self.timeline.restart(id, cursor, to, rate);
    }

    fn set_volume(&mut self, level: f32) {
        self.volume = level.clamp(0.0, 1.0);
        let Some(backend) = &self.backend else {
            return;
        };
        backend.set_volume(self.volume);
        let software = backend.software_volume(self.volume);
        for playing in [&mut self.decoding, &mut self.previous]
            .into_iter()
            .flatten()
        {
            playing.pipeline.set_volume(software);
        }
        self.report_path();
    }

    fn set_replay_gain(&mut self, replay_gain: ReplayGain) {
        self.replay_gain = replay_gain;
        let software = self
            .backend
            .as_ref()
            .and_then(|backend| backend.software_volume(self.volume));
        for playing in [&mut self.decoding, &mut self.previous]
            .into_iter()
            .flatten()
        {
            playing.update_gains(software, &self.replay_gain);
        }
        self.report_path();
    }

    fn choose(&mut self, output: Output) {
        if self.choice == output {
            return;
        }
        self.choice = output;
        self.exclusive_allowed = true;
        if self.decoding.is_some() {
            self.relocate();
        } else {
            self.backend = None;
        }
    }
}
