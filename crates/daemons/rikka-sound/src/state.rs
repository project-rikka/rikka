use futures_util::Stream;

use crate::command::Command;
use crate::node::NodeKind;
use crate::peaks::Peaks;
use crate::service::AudioService;
use crate::snapshot::Snapshot;
use crate::volume::Volume;
use rikka_bus::Monitor;

#[derive(Clone, Debug)]
pub struct Sound(Monitor<AudioService>);

impl Sound {
    #[must_use]
    pub fn start() -> Sound {
        Sound(Monitor::start())
    }

    #[must_use]
    pub fn state(&self) -> Snapshot {
        self.0.state()
    }

    pub fn changes(&self) -> impl Stream<Item = Snapshot> + Send + 'static {
        self.0.changes()
    }

    #[must_use]
    pub fn unavailable(&self) -> Option<String> {
        self.0.unavailable()
    }

    #[must_use]
    pub fn peaks(&self) -> Option<&Peaks> {
        self.0.backend().map(AudioService::peaks)
    }

    pub fn send(&self, command: Command) {
        self.0.request(command);
    }

    pub fn set_level(&self, node: u32, level: f32) {
        self.retune(node, |volume| volume.at_level(level));
    }

    pub fn mute(&self, node: u32, muted: bool) {
        self.retune(node, |volume| volume.muted_to(muted));
    }

    pub fn use_device(&self, kind: NodeKind, name: &str) {
        self.0.request(Command::Default {
            kind,
            name: name.to_owned(),
        });
    }

    pub fn allow_rates(&self, rates: &[u32]) {
        let allowed = rates.to_vec();
        self.0.edit(|graph| {
            let moved = graph.graph.allowed_rates != allowed;
            graph.graph.allowed_rates.clone_from(&allowed);
            moved
        });
        self.0.request(Command::AllowRates(allowed));
    }

    fn retune(&self, node: u32, tune: impl Fn(&Volume) -> Volume) {
        let mut tuned = None;
        self.0.edit(|graph| {
            let Some(entry) = graph.nodes.iter_mut().find(|entry| entry.id == node) else {
                return false;
            };
            let volume = tune(&entry.volume);
            let moved = entry.volume != volume;
            entry.volume = volume.clone();
            tuned = Some(volume);
            moved
        });
        if let Some(volume) = tuned {
            self.0.request(Command::Volume { node, volume });
        }
    }
}
