use std::time::Duration;

use rikka_settings::{Key, Schema};
use rikka_types::Choice;

use crate::preferences::Preferences;

const MEMORY_SAVER: &str = "memory-saver";
const NEVER_DISCARD: &str = "memory-saver-exceptions";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum MemorySaver {
    Off,
    Quarter,
    #[default]
    Hour,
    FourHours,
}

impl Choice for MemorySaver {
    const ALL: &'static [MemorySaver] = &[
        MemorySaver::Off,
        MemorySaver::Quarter,
        MemorySaver::Hour,
        MemorySaver::FourHours,
    ];

    fn slug(self) -> &'static str {
        match self {
            MemorySaver::Off => "off",
            MemorySaver::Quarter => "15m",
            MemorySaver::Hour => "1h",
            MemorySaver::FourHours => "4h",
        }
    }
}

impl MemorySaver {
    pub(crate) fn idle(self) -> Option<Duration> {
        let minutes = match self {
            MemorySaver::Off => return None,
            MemorySaver::Quarter => 15,
            MemorySaver::Hour => 60,
            MemorySaver::FourHours => 4 * 60,
        };
        Some(Duration::from_secs(minutes * 60))
    }
}

impl Preferences {
    pub(crate) fn memory_saver(&self) -> MemorySaver {
        self.observers.read();
        self.settings.borrow().choice(MEMORY_SAVER)
    }

    pub(crate) fn set_memory_saver(&self, saver: MemorySaver) {
        self.write(|settings| settings.set_choice(MEMORY_SAVER, saver));
    }

    pub(crate) fn never_discarded(&self) -> Vec<String> {
        self.observers.read();
        self.settings.borrow().get::<Vec<String>>(NEVER_DISCARD)
    }

    pub(crate) fn set_never_discarded(&self, hosts: Vec<String>) {
        self.put::<Vec<String>>(NEVER_DISCARD, hosts);
    }
}

pub(super) fn keys(schema: Schema) -> Schema {
    schema
        .key(Key::choice::<MemorySaver>(MEMORY_SAVER).summary("Discard tabs left unused for"))
        .key(Key::list(NEVER_DISCARD, []).summary("Sites whose tabs are never discarded"))
}
