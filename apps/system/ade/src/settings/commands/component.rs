use std::path::PathBuf;

use ade_scope::Scope;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::notice::notice;
use crate::settings::commands::card::card;
use crate::settings::diagnostic::diagnostics_notice;
use crate::settings::rows::{Located, located};
use crate::settings::scope::whose;

#[derive(Debug, Default)]
pub(crate) struct Commands {
    folders: Vec<Located>,
    catalog: ade_command::Commands,
}

impl Commands {
    pub(crate) fn reread(&mut self, root: Option<PathBuf>) {
        let root = root.as_deref();
        self.folders = Scope::PARTICULAR_FIRST
            .map(|scope| Located::find(scope, ade_command::Commands::folder(scope, root)))
            .to_vec();
        self.catalog = ade_command::Commands::find(root);
    }
}

pub(crate) fn view(commands: &Commands) -> impl View {
    column! {
        folders(commands);
        command_list(commands);
        diagnostics_notice(tr!("ade.settings.commands.trouble"), commands.catalog.diagnostics());
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn folders(commands: &Commands) -> impl View {
    let rows = column! {
        for folder in &commands.folders {
            match folder.path() {
                Some(at) => located(
                    MaterialSymbol::FOLDER_OPEN,
                    MaterialSymbol::FOLDER_OFF,
                    text(whose(folder.scope)).caption(),
                    at,
                    folder.is_present(),
                )
                .any(),
                None => notice(
                    MaterialSymbol::INFO,
                    tr!("ade.settings.commands.projectless"),
                    palette().text_muted,
                )
                .any(),
            }
        }
    }
    .gap(Space::TIGHT)
    .fill_width();

    section(tr!("ade.settings.commands.folders"), rows)
}

fn command_list(commands: &Commands) -> Container {
    if commands.catalog.is_empty() {
        return section(
            tr!("ade.settings.commands.found"),
            text(tr!("ade.settings.commands.none")).caption().muted(),
        );
    }

    let rows = column! {
        for (nth, command) in commands.catalog.all().iter().enumerate() {
            if nth > 0 {
                rule();
            }
            card(command);
        }
    }
    .gap(Space::TIGHT)
    .fill_width();

    section(tr!("ade.settings.commands.found"), rows)
}
