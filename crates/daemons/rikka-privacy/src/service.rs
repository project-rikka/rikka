use rikka_sync::Mutex;
use std::sync::Arc;

use std::convert::Infallible;

use rikka_bus::{Backend, EventSink, StartError, Unavailable};
use rikka_worker::Worker;

use crate::graph::Users;
use crate::pipewire;
use crate::user::{State, User, deduplicated};
use crate::v4l2::{self, Ring};

#[derive(Debug, Default)]
struct Parts {
    graph: Users,
    devices: Vec<User>,
}

impl Parts {
    fn merged(&self) -> State {
        State {
            microphone: self.graph.microphone.clone(),
            camera: deduplicated(
                self.graph
                    .camera
                    .iter()
                    .chain(&self.devices)
                    .cloned()
                    .collect(),
            ),
            screen: self.graph.screen.clone(),
        }
    }
}

pub(crate) struct Watchers {
    _pipewire: Option<rikka_pipewire::LoopWorker>,
    _cameras: Option<Worker<Ring>>,
}

impl Backend for Watchers {
    type Request = Infallible;
    type Event = State;
    type State = State;

    const UNAVAILABLE: &'static str = "this machine cannot tell who is listening or watching";

    fn start(report: EventSink<State>, _: Unavailable) -> Result<Watchers, StartError> {
        let parts = Arc::new(Mutex::new(Parts::default()));
        let tell = move |change: Box<dyn FnOnce(&mut Parts) + Send>| {
            let merged = {
                let mut parts = parts.lock();
                change(&mut parts);
                parts.merged()
            };
            report.emit(merged);
        };
        let telling = tell.clone();
        let pipewire = pipewire::start(Box::new(move |update| {
            telling(Box::new(move |parts| parts.graph = update));
        }))
        .inspect_err(|error| tracing::debug!("failed to watch PipeWire capture streams: {error}"))
        .ok();
        let cameras = v4l2::start(Box::new(move |users| {
            tell(Box::new(move |parts| parts.devices = users));
        }))
        .inspect_err(|error| tracing::debug!("failed to watch V4L2 devices: {error}"))
        .ok();
        if pipewire.is_none() && cameras.is_none() {
            return Err("neither PipeWire nor the cameras can be watched".into());
        }
        Ok(Watchers {
            _pipewire: pipewire,
            _cameras: cameras,
        })
    }

    fn request(&self, request: Infallible) {
        match request {}
    }
}
