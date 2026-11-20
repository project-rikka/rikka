mod calls;
mod codec;
mod replayer;
mod scene;
mod values;

pub use replayer::{Divergence, ReplayReport, Replayer};

use crate::Result;
use crate::call::Call;
use crate::history::{self, Setup};
use crate::settings::Settings;

#[derive(Clone, Debug)]
pub(crate) enum Entry {
    Setup(Setup),
    Call(Call),
    Tick(u64),
}

#[derive(Clone, Debug)]
pub struct Recording {
    settings: Settings,
    entries: Vec<Entry>,
}

impl Recording {
    pub(crate) fn new(settings: Settings, scene: &[history::Entry]) -> Self {
        let entries = scene
            .iter()
            .map(|entry| match entry {
                history::Entry::Setup(setup) => Entry::Setup(setup.clone()),
                history::Entry::Call(call) => Entry::Call(call.clone()),
            })
            .collect();
        Self { settings, entries }
    }

    pub(crate) fn push_setup(&mut self, setup: Setup) {
        self.entries.push(Entry::Setup(setup));
    }

    pub(crate) fn push_call(&mut self, call: Call) {
        self.entries.push(Entry::Call(call));
    }

    pub(crate) fn push_tick(&mut self, hash: u64) {
        self.entries.push(Entry::Tick(hash));
    }

    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub(crate) fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn ticks(&self) -> u64 {
        self.entries
            .iter()
            .filter(|entry| matches!(entry, Entry::Tick(_)))
            .count() as u64
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        codec::FORMAT.encode(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        codec::FORMAT.decode(bytes)
    }

    pub fn replayer(&self) -> Result<Replayer<'_>> {
        Replayer::new(self)
    }

    pub fn replay(&self) -> Result<ReplayReport> {
        self.replayer()?.run()
    }
}
