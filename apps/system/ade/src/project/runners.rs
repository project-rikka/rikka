use std::collections::HashMap;

use rikka_app::prelude::*;

use crate::project::runner::Runner;

#[derive(Default, State)]
pub(crate) struct Runners {
    #[untracked]
    runners: HashMap<String, Runner>,
    roster: Revision,
}

impl Runners {
    pub(crate) fn get(&self, thread: &str) -> Option<&Runner> {
        self.roster.read();
        self.runners.get(thread)
    }

    pub(crate) fn get_mut(&mut self, thread: &str) -> Option<&mut Runner> {
        self.runners.get_mut(thread)
    }

    pub(crate) fn insert(&mut self, thread: String, runner: Runner) -> Option<Runner> {
        self.roster.changed();
        self.runners.insert(thread, runner)
    }

    pub(crate) fn remove(&mut self, thread: &str) -> Option<Runner> {
        self.roster.changed();
        self.runners.remove(thread)
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut Runner> {
        self.runners.values_mut()
    }
}

impl std::fmt::Debug for Runners {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runners")
            .field("runners", &self.runners.len())
            .finish_non_exhaustive()
    }
}
