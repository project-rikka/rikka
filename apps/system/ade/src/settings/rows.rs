use std::path::{Path, PathBuf};

use ade_scope::Scope;
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use crate::settings::scope::whose;

const MARK: f32 = 16.0;

#[derive(Clone, Debug)]
pub(crate) struct Located {
    pub(crate) scope: Scope,
    at: Option<PathBuf>,
    present: bool,
}

impl Located {
    pub(crate) fn find(scope: Scope, at: Option<PathBuf>) -> Located {
        Located {
            scope,
            present: at.as_deref().is_some_and(Path::exists),
            at,
        }
    }

    pub(crate) fn path(&self) -> Option<&Path> {
        self.at.as_deref()
    }

    pub(crate) fn is_present(&self) -> bool {
        self.present
    }
}

pub(crate) fn located(
    there: MaterialSymbol,
    waiting: MaterialSymbol,
    content: impl View,
    path: &Path,
    present: bool,
) -> impl View {
    row! {
        icon(match present {
            true => there,
            false => waiting,
        })
        .extent(Length::px(MARK))
        .color(palette().text_muted);
        content;
        text(path.display().to_string()).caption().muted().grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

pub(crate) fn entry(name: String, scope: Scope, path: &Path) -> impl View {
    column! {
        row! {
            text(name).label().medium().grow(1.0);
            text(whose(scope)).caption().muted();
        }
        .gap(Space::SNUG)
        .align_items(Align::Center)
        .fill_width();
        text(path.display().to_string()).caption().muted();
    }
    .gap(Space::HAIR)
    .fill_width()
}
