mod backend;
mod command;
mod commands;
mod feeding;
mod outputs;
mod playing;
mod reporting;
mod timeline;

use std::os::fd::AsFd;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use rikka_hifi_core::{ReplayGain, SignalPath};
use rikka_worker::{Broadcast, CommandReceiver, EventFd, event_fd};

use crate::event::{Event, OutputState};
use crate::exclusive::reserve::Bus;
use crate::identity::Identity;
use crate::notice::{Notifier, OutputNotice};
use crate::output::Output;
use crate::source::SourceStream;
use backend::Backend;
use playing::Playing;
use timeline::Timeline;

pub(crate) use command::Command;

pub(crate) const NAME: &str = "rikka-hifi";

const TICK: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Idle,
    Playing,
    Paused,
}

pub(crate) struct Engine {
    identity: Identity,
    commands: CommandReceiver<Command>,
    woken: EventFd,
    notifier: Notifier,
    notices: Receiver<OutputNotice>,
    subscribers: Broadcast<Event>,
    choice: Output,
    exclusive_allowed: bool,
    outputs: u64,
    backend: Option<Backend>,
    bus: Option<Bus>,
    decoding: Option<Playing>,
    previous: Option<Playing>,
    next: Option<SourceStream>,
    waiting: bool,
    timeline: Timeline,
    state: State,
    volume: f32,
    replay_gain: ReplayGain,
    pending: Vec<u8>,
    pcm: Vec<f32>,
    dsd: Vec<u8>,
    told_path: Option<SignalPath>,
    told_output: Option<OutputState>,
    told_position: Instant,
}

impl Engine {
    pub(crate) fn new(
        identity: Identity,
        commands: CommandReceiver<Command>,
    ) -> std::io::Result<Engine> {
        let (wake, woken) = event_fd()?;
        let (notifier, notices) = Notifier::open(wake);
        Ok(Engine {
            identity,
            commands,
            woken,
            notifier,
            notices,
            subscribers: Broadcast::new(),
            choice: Output::Shared,
            exclusive_allowed: true,
            outputs: 0,
            backend: None,
            bus: None,
            decoding: None,
            previous: None,
            next: None,
            waiting: false,
            timeline: Timeline::default(),
            state: State::Idle,
            volume: 1.0,
            replay_gain: ReplayGain::default(),
            pending: Vec::new(),
            pcm: Vec::new(),
            dsd: Vec::new(),
            told_path: None,
            told_output: None,
            told_position: Instant::now(),
        })
    }

    pub(crate) fn run(mut self) {
        loop {
            self.woken.drain();
            let commands: Vec<Command> = self.commands.drain().collect();
            for command in commands {
                if !self.handle_command(command) {
                    return;
                }
            }
            let notices: Vec<OutputNotice> = self.notices.try_iter().collect();
            for notice in notices {
                self.handle_notice(notice);
            }
            self.feed();
            self.report();
            if let Err(error) = self.commands.wait_readable(self.woken.as_fd(), TICK) {
                tracing::error!("hifi engine wait failed: {error}");
                return;
            }
        }
    }

    fn emit(&mut self, event: Event) {
        self.subscribers.send(&event);
    }

    fn next_output(&mut self) -> Notifier {
        self.outputs += 1;
        self.notifier.for_output(self.outputs)
    }
}
