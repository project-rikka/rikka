use rikka_physics_rigid::{Body, BodyId, JointDef, ShapeDef};

use crate::CharacterDef;
use crate::call::Call;

#[derive(Clone, Debug)]
pub(crate) enum Setup {
    Body(Body),
    Shape(BodyId, ShapeDef),
    Joint(Box<JointDef>),
    Character(CharacterDef),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    Structure,
    State,
    Input,
}

#[derive(Clone, Debug)]
pub(crate) enum Entry {
    Setup(Setup),
    Call(Call),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct History {
    entries: Vec<Entry>,
    started: bool,
}

impl History {
    pub(crate) fn setup(&mut self, setup: Setup) {
        self.entries.push(Entry::Setup(setup));
    }

    pub(crate) fn wants(&self, scope: Scope) -> bool {
        match scope {
            Scope::Structure => true,
            Scope::State => !self.started,
            Scope::Input => false,
        }
    }

    pub(crate) fn call(&mut self, call: Call) {
        self.entries.push(Entry::Call(call));
    }

    pub(crate) fn start(&mut self) {
        self.started = true;
    }

    pub(crate) fn is_started(&self) -> bool {
        self.started
    }

    pub(crate) fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub(crate) fn take(&mut self) -> Vec<Entry> {
        std::mem::take(&mut self.entries)
    }

    pub(crate) fn restore(&mut self, entries: Vec<Entry>) {
        self.entries = entries;
    }
}
