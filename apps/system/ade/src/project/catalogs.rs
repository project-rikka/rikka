use std::path::Path;

use ade_command::Commands;
use ade_guide::Guides;
use ade_memory::Memories;
use ade_skill::Skills;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Catalogs {
    pub(crate) skills: Skills,
    pub(crate) commands: Commands,
    pub(crate) guides: Guides,
    pub(crate) memories: Memories,
}

impl Catalogs {
    pub(crate) fn find(root: &Path) -> Catalogs {
        let catalogs = Catalogs {
            skills: Skills::find(Some(root)),
            commands: Commands::find(Some(root)),
            guides: Guides::find(Some(root)),
            memories: Memories::find(Some(root)),
        };
        if let Err(error) = catalogs.memories.write_index() {
            tracing::warn!(%error, "failed to write memory index");
        }
        catalogs
    }

    pub(crate) fn report(&self) {
        let diagnostics = self
            .skills
            .diagnostics()
            .iter()
            .chain(self.commands.diagnostics())
            .chain(self.guides.diagnostics())
            .chain(self.memories.diagnostics());
        for diagnostic in diagnostics {
            tracing::info!(
                path = %diagnostic.path().display(),
                "{}",
                diagnostic.message()
            );
        }
    }
}
