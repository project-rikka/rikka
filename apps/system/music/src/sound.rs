use std::sync::Arc;

use music_core::Fidelity;
use rikka_app::prelude::*;
use rikka_hifi::{Device, Event, Hifi, Output, OutputState, ReplayGain, SignalPath};
use rikka_sound::{NodeKind, Snapshot};

const EVENTS: &str = "music-output-events";
const DEVICES: &str = "music-output-devices";
const GRAPH: &str = "music-output-graph";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Sink {
    pub(crate) name: String,
    pub(crate) title: String,
}

#[derive(State)]
pub(crate) struct Sound {
    #[untracked]
    hifi: Hifi,
    #[untracked]
    graph: Option<rikka_sound::Sound>,
    path: Tracked<Option<SignalPath>>,
    state: Tracked<Option<OutputState>>,
    devices: Tracked<Arc<[Device]>>,
    sinks: Tracked<Arc<[Sink]>>,
    output: Tracked<Output>,
    replay_gain: Tracked<ReplayGain>,
}

impl Sound {
    pub(crate) fn new(hifi: Hifi, output: Output, replay_gain: ReplayGain) -> Sound {
        hifi.set_output(output.clone());
        hifi.set_replay_gain(replay_gain);
        Sound {
            hifi,
            graph: None,
            path: Tracked::new(None),
            state: Tracked::new(None),
            devices: Tracked::new(Arc::from([])),
            sinks: Tracked::new(Arc::from([])),
            output: Tracked::new(output),
            replay_gain: Tracked::new(replay_gain),
        }
    }

    pub(crate) fn listen(&self) {
        stream_keyed(
            EVENTS,
            self.hifi.events(),
            |sound: &mut Sound, event: Event| {
                sound.handle_event(event);
            },
        );
    }

    pub(crate) fn list_devices(&self) {
        spawn_blocking_keyed(
            DEVICES,
            Hifi::devices,
            |sound: &mut Sound, listed: rikka_hifi::Result<Vec<Device>>| match listed {
                Ok(devices) => sound.devices.set(devices.into()),
                Err(error) => {
                    tracing::warn!("failed to list sound cards: {error}");
                    skip_view();
                }
            },
        );
    }

    pub(crate) fn watch_graph(&mut self) {
        if self.graph.is_some() {
            return;
        }
        let graph = rikka_sound::Sound::start();
        self.sinks.set(sinks_of(&graph.state()));
        stream_keyed(
            GRAPH,
            graph.changes(),
            |sound: &mut Sound, snapshot: Snapshot| sound.apply_graph(&snapshot),
        );
        self.graph = Some(graph);
    }

    pub(crate) fn allow_rates(&mut self, rates: &[u32]) {
        self.watch_graph();
        if let Some(graph) = &self.graph {
            graph.allow_rates(rates);
        }
    }

    pub(crate) fn path(&self) -> Option<&SignalPath> {
        self.path.as_ref()
    }

    pub(crate) fn fidelity(&self) -> Option<Fidelity> {
        self.path
            .as_ref()
            .map(|path| Fidelity::from_source_format(&path.source))
    }

    pub(crate) fn state(&self) -> Option<&OutputState> {
        self.state.as_ref()
    }

    pub(crate) fn devices(&self) -> &[Device] {
        &self.devices
    }

    pub(crate) fn sinks(&self) -> &[Sink] {
        &self.sinks
    }

    pub(crate) fn output(&self) -> &Output {
        &self.output
    }

    pub(crate) fn replay_gain(&self) -> ReplayGain {
        *self.replay_gain
    }

    pub(crate) fn choose(&mut self, output: Output) {
        if *self.output == output {
            return;
        }
        self.hifi.set_output(output.clone());
        self.output.set(output);
    }

    pub(crate) fn level(&mut self, replay_gain: ReplayGain) {
        if *self.replay_gain == replay_gain {
            return;
        }
        self.hifi.set_replay_gain(replay_gain);
        self.replay_gain.set(replay_gain);
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Path(path) => self.path.set(Some(*path)),
            Event::Output(state) => self.state.set(Some(state)),
            _ => skip_view(),
        }
    }

    fn apply_graph(&mut self, snapshot: &Snapshot) {
        let sinks = sinks_of(snapshot);
        if *self.sinks == sinks {
            skip_view();
            return;
        }
        self.sinks.set(sinks);
    }
}

fn sinks_of(snapshot: &Snapshot) -> Arc<[Sink]> {
    snapshot
        .nodes_of_kind(NodeKind::Sink)
        .map(|node| Sink {
            name: node.name.clone(),
            title: match node.title.is_empty() {
                true => node.name.clone(),
                false => node.title.clone(),
            },
        })
        .collect()
}

impl std::fmt::Debug for Sound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Sound")
            .field("output", &*self.output)
            .field("state", &*self.state)
            .finish_non_exhaustive()
    }
}
