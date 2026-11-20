mod catalogue;
mod shelf;

use rikka_ui::{MaterialSymbol, tr};
use rikka_xdg::Exec;
use shelf::Shelf;
use shell_kit::launch::Spawn;

use crate::provider::{Activation, CERTAIN, ChangeStream, Hit, Provider, ProviderInfo, Query};

pub(crate) const ID: &str = "run";

const TRIGGER: &str = ">";

const LIMIT: usize = 12;

const COMMAND: &str = "shell.launcher.run.command";

#[derive(Debug, Default)]
pub(super) struct RunProvider {
    shelf: Shelf,
}

impl Provider for RunProvider {
    fn info(&self) -> ProviderInfo {
        ProviderInfo::new(ID, tr!("shell.launcher.provider.programs"))
            .icon(MaterialSymbol::TERMINAL)
            .trigger(TRIGGER)
    }

    fn reload(&mut self) {
        self.shelf.rescan();
    }

    fn search(&mut self, query: &Query) -> Vec<Hit> {
        self.shelf.take_scan();

        let line = query.text();
        if !query.is_forced() || line.is_empty() {
            return Vec::new();
        }

        let mut hits = vec![Hit::new(line, line).subtitle(tr!(COMMAND)).score(CERTAIN)];

        if !line.contains(char::is_whitespace) {
            hits.extend(
                self.shelf
                    .search(query, LIMIT + 1)
                    .into_iter()
                    .filter(|hit| hit.id() != line)
                    .take(LIMIT),
            );
        }
        hits
    }

    fn recall(&mut self, id: &str) -> Option<Hit> {
        let program = Exec::words(id).into_iter().next()?;
        self.shelf
            .knows(&program)
            .then(|| Hit::new(id, id).subtitle(tr!(COMMAND)))
    }

    fn activate(&mut self, hit: &Hit) -> Activation {
        let argv = Exec::words(hit.id());
        if argv.is_empty() {
            return Activation::Ignore;
        }
        Activation::spawn(Spawn::new(argv))
    }

    fn changes(&self) -> Option<ChangeStream> {
        Some(self.shelf.changes())
    }
}
