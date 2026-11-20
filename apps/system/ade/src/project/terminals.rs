use std::collections::HashMap;
use std::path::Path;

use rikka_app::prelude::*;
use rikka_terminal::Report;

use crate::project::handle::ProjectHandle;
use crate::project::terminal::Terminal;

#[derive(State)]
pub(crate) struct Terminals {
    #[untracked]
    handle: ProjectHandle,
    #[untracked]
    terminals: HashMap<String, Terminal>,
    roster: Revision,
}

impl Terminals {
    pub(crate) fn new(handle: &ProjectHandle) -> Terminals {
        Terminals {
            handle: handle.clone(),
            terminals: HashMap::new(),
            roster: Revision::new(),
        }
    }

    pub(crate) fn get(&self, thread: &str) -> Option<&Terminal> {
        self.roster.read();
        self.terminals.get(thread)
    }

    pub(crate) fn raise(
        &mut self,
        thread: &str,
        at: &Path,
        act: impl FnOnce(&mut Terminal, &Path),
    ) {
        let Terminals {
            handle,
            terminals,
            roster,
        } = self;
        let terminal = terminals.entry(thread.to_owned()).or_insert_with(|| {
            roster.changed();
            Terminal::new(handle)
        });
        act(terminal, at);
        self.prune(thread);
    }

    pub(crate) fn act(&mut self, thread: &str, act: impl FnOnce(&mut Terminal)) {
        let Some(terminal) = self.terminals.get_mut(thread) else {
            skip_view();
            return;
        };
        act(terminal);
        self.prune(thread);
    }

    pub(crate) fn follow(&mut self, thread: &str, at: &Path) {
        if let Some(terminal) = self.terminals.get_mut(thread) {
            terminal.follow(at);
            self.prune(thread);
        }
    }

    pub(crate) fn report(&mut self, serial: u64, report: Report) {
        let Some((thread, terminal)) = self
            .terminals
            .iter_mut()
            .find(|(_, terminal)| terminal.runs(serial))
        else {
            skip_view();
            return;
        };
        terminal.report(report);
        let thread = thread.clone();
        self.prune(&thread);
    }

    pub(crate) fn rekey(&mut self, from: &str, to: &str) {
        if let Some(terminal) = self.terminals.remove(from) {
            self.roster.changed();
            self.terminals.insert(to.to_owned(), terminal);
        }
    }

    pub(crate) fn close(&mut self, thread: &str) {
        if self.terminals.remove(thread).is_some() {
            self.roster.changed();
        }
    }

    fn prune(&mut self, thread: &str) {
        if self
            .terminals
            .get(thread)
            .is_some_and(|terminal| !terminal.is_live())
        {
            self.close(thread);
        }
    }
}

impl std::fmt::Debug for Terminals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminals")
            .field("terminals", &self.terminals.len())
            .finish_non_exhaustive()
    }
}
