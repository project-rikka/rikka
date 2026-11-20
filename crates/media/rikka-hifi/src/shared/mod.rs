mod graph;
mod pods;
mod stream;

use std::ops::ControlFlow;
use std::rc::Rc;

use rikka_hifi_core::Plan;
use rikka_pipewire::Connection;
use rikka_worker::{Stop, Worker};

use crate::identity::Identity;
use crate::notice::Notifier;
use crate::ring::{self, RingReader, RingWriter};
use graph::{Graph, with};

pub(crate) const NAME: &str = "rikka-hifi-pipewire";

const BUFFERED_SECONDS_DIVISOR: usize = 4;

enum Command {
    Active(bool),
    Volume(f32),
    Quit,
}

struct QuitSender(pipewire::channel::Sender<Command>);

impl Stop for QuitSender {
    fn stop(&self) {
        let _ = self.0.send(Command::Quit);
    }
}

pub(crate) struct Setup {
    pub(crate) identity: Identity,
    pub(crate) sink: Option<String>,
    pub(crate) plan: Plan,
    pub(crate) volume: f32,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Unavailable {
    #[error("failed to allocate the output ring: {0}")]
    Ring(#[source] std::io::Error),

    #[error("failed to start the PipeWire stream: {0}")]
    Start(#[from] rikka_worker::Error<pipewire::Error>),
}

pub(crate) struct SharedOutput {
    plan: Plan,
    ring: RingWriter,
    commands: pipewire::channel::Sender<Command>,
    _worker: Worker<QuitSender>,
}

impl SharedOutput {
    pub(crate) fn open(setup: Setup, notifier: Notifier) -> Result<SharedOutput, Unavailable> {
        let plan = setup.plan;
        let frames = plan.rate as usize / BUFFERED_SECONDS_DIVISOR;
        let opened = ring::open(frames, plan.frame_bytes(), notifier.wake().clone())
            .map_err(Unavailable::Ring)?;
        let (commands, arriving) = pipewire::channel::channel();
        let reader = opened.reader;
        let worker = Worker::spawn(NAME, QuitSender(commands.clone()), move || {
            run(setup, reader, notifier, arriving)
        })?;
        Ok(SharedOutput {
            plan,
            ring: opened.writer,
            commands,
            _worker: worker,
        })
    }

    pub(crate) fn plan(&self) -> Plan {
        self.plan
    }

    pub(crate) fn ring(&self) -> &RingWriter {
        &self.ring
    }

    pub(crate) fn ring_mut(&mut self) -> &mut RingWriter {
        &mut self.ring
    }

    pub(crate) fn set_active(&self, active: bool) {
        let _ = self.commands.send(Command::Active(active));
    }

    pub(crate) fn set_volume(&self, volume: f32) {
        let _ = self.commands.send(Command::Volume(volume));
    }
}

impl std::fmt::Debug for SharedOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedOutput")
            .field("plan", &self.plan)
            .finish_non_exhaustive()
    }
}

fn run(
    setup: Setup,
    ring: RingReader,
    notifier: Notifier,
    commands: pipewire::channel::Receiver<Command>,
) -> Result<impl FnOnce(), pipewire::Error> {
    let connection = Connection::open()?;
    let graph = Graph::new(
        connection.registry().clone(),
        setup.plan.rate,
        setup.plan.channels,
        setup.volume,
        notifier.clone(),
    );
    let playing =
        stream::Playing::connect(connection.core(), &setup, ring, &graph, notifier.clone())?;
    Ok(move || {
        let appearing = Rc::downgrade(&graph);
        let vanishing = Rc::downgrade(&graph);
        let _registry = connection
            .registry()
            .add_listener_local()
            .global(move |global| with(&appearing, |graph| graph.added(global, &appearing)))
            .global_remove(move |id| with(&vanishing, |graph| graph.removed(id, &vanishing)))
            .register();
        let failing = notifier;
        let _core = connection
            .core()
            .add_listener_local()
            .error(move |id, _, _, message| {
                if id == pipewire::core::PW_ID_CORE {
                    failing.send(crate::notice::Notice::SharedFailed(message.to_owned()));
                }
            })
            .register();
        let commanded = Rc::downgrade(&graph);
        connection.run_with(commands, move |command| {
            match command {
                Command::Quit => return ControlFlow::Break(()),
                Command::Active(active) => playing.set_active(active),
                Command::Volume(volume) => {
                    playing.set_volume(volume);
                    with(&commanded, |graph| graph.set_volume(volume));
                }
            }
            ControlFlow::Continue(())
        });
    })
}
