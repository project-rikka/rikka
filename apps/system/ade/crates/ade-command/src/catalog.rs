use std::path::{Path, PathBuf};
use std::sync::Arc;

use ade_scope::{Diagnostic, Scope, listing};

use crate::command::Command;

const COMMANDS: &str = "commands";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Commands {
    commands: Vec<Arc<Command>>,
    diagnostics: Vec<Diagnostic>,
}

impl Commands {
    #[must_use]
    pub fn find(project: Option<&Path>) -> Commands {
        let mut catalog = Commands::default();

        for scope in Scope::PARTICULAR_FIRST {
            let Some(folder) = Commands::folder(scope, project) else {
                continue;
            };
            for template in listing::markdown_files(&folder) {
                catalog.take(&template, scope);
            }
        }

        catalog
            .commands
            .sort_by(|one, two| one.name().cmp(two.name()));
        catalog
    }

    #[must_use]
    pub fn folder(scope: Scope, project: Option<&Path>) -> Option<PathBuf> {
        Some(scope.rikka(project)?.join(COMMANDS))
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Command> {
        self.commands
            .iter()
            .find(|command| same(command.name(), name))
            .map(Arc::as_ref)
    }

    #[must_use]
    pub fn matching(&self, typed: &str) -> Vec<Arc<Command>> {
        self.commands
            .iter()
            .filter(|command| starts(command.name(), typed))
            .map(Arc::clone)
            .collect()
    }

    #[must_use]
    pub fn all(&self) -> &[Arc<Command>] {
        &self.commands
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    fn take(&mut self, template: &Path, scope: Scope) {
        let command = match Command::read(template, scope) {
            Ok(read) => read,
            Err(reason) => {
                return self
                    .diagnostics
                    .push(Diagnostic::skipped(template, reason.to_string()));
            }
        };

        if let Some(had) = self.get(command.name()) {
            let message = format!(
                "\"{}\" is already a command, found at {}",
                command.name(),
                had.path().display()
            );
            return self
                .diagnostics
                .push(Diagnostic::skipped(template, message));
        }

        self.commands.push(Arc::new(command));
    }
}

fn folded(name: &str) -> impl Iterator<Item = char> {
    name.chars().flat_map(char::to_lowercase)
}

fn same(name: &str, query: &str) -> bool {
    folded(name).eq(folded(query))
}

fn starts(name: &str, typed: &str) -> bool {
    let mut name = folded(name);
    folded(typed).all(|letter| name.next() == Some(letter))
}
