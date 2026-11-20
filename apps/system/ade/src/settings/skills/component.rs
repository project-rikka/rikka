use std::path::PathBuf;

use ade_scope::Scope;
use rikka_app::prelude::*;
use rikka_widgets::column;

use crate::notice::notice;
use crate::settings::diagnostic::diagnostics_notice;
use crate::settings::rows::{Located, located};
use crate::settings::scope::whose;
use crate::settings::skills::card::card;

#[derive(Debug, Default)]
pub(crate) struct Skills {
    folders: Vec<Located>,
    catalog: ade_skill::Skills,
}

impl Skills {
    pub(crate) fn reread(&mut self, root: Option<PathBuf>) {
        let root = root.as_deref();
        self.folders = Scope::PARTICULAR_FIRST
            .map(|scope| Located::find(scope, ade_skill::Skills::folder(scope, root)))
            .to_vec();
        self.catalog = ade_skill::Skills::find(root);
    }
}

pub(crate) fn view(skills: &Skills) -> impl View {
    column! {
        folders(skills);
        skill_list(skills);
        diagnostics_notice(tr!("ade.settings.skills.trouble"), skills.catalog.diagnostics());
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn folders(skills: &Skills) -> impl View {
    let rows = column! {
        for folder in &skills.folders {
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
                    tr!("ade.settings.skills.projectless"),
                    palette().text_muted,
                )
                .any(),
            }
        }
    }
    .gap(Space::TIGHT)
    .fill_width();

    section(tr!("ade.settings.skills.folders"), rows)
}

fn skill_list(skills: &Skills) -> Container {
    if skills.catalog.is_empty() {
        return section(
            tr!("ade.settings.skills.found"),
            text(tr!("ade.settings.skills.none")).caption().muted(),
        );
    }

    let rows = column! {
        for (nth, skill) in skills.catalog.all().iter().enumerate() {
            if nth > 0 {
                rule();
            }
            card(skill);
        }
    }
    .gap(Space::TIGHT)
    .fill_width();

    section(tr!("ade.settings.skills.found"), rows)
}
