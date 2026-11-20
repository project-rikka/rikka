use std::path::Path;

use rikka_app::prelude::*;

use crate::project::act::given;
use crate::project::palette::{self, Choice, Palette};
use crate::project::tab::Tab;

use super::Project;

const PALETTE: &str = "ade-palette";

impl Project {
    pub(in crate::project) fn palette(&self) -> Option<&Palette> {
        self.palette.as_ref()
    }

    pub(in crate::project) fn summon_palette(&mut self) {
        if let Some(workspace) = self.shown_workspace() {
            workspace.freshen();
        }
        *self.palette = Some(Palette::new());
        self.palette_typed();
    }

    pub(in crate::project) fn dismiss_palette(&mut self) {
        match self.palette.is_some() {
            true => *self.palette = None,
            false => skip_view(),
        }
    }

    pub(in crate::project) fn palette_typed(&mut self) {
        let workspace = self.shown();
        let root = workspace.root().to_owned();
        let paths = workspace.paths();
        let active = workspace
            .viewer()
            .shown()
            .map(|shown| shown.path().to_owned());
        let recent: Vec<String> = workspace
            .viewer()
            .recent()
            .iter()
            .filter(|path| active.as_deref() != Some(path.as_path()))
            .filter_map(|path| relative(&root, path))
            .collect();
        let Some(request) = self.palette.as_mut().map(Palette::ask) else {
            skip_view();
            return;
        };
        let (request, query) = request;
        spawn_blocking_keyed(
            PALETTE,
            move || palette::choices(&paths, recent, &query),
            given(move |project, choices: Vec<Choice>| project.palette_answered(request, choices)),
        );
    }

    fn palette_answered(&mut self, request: u64, choices: Vec<Choice>) {
        let answered = self
            .palette
            .as_mut()
            .is_some_and(|palette| palette.answered(request, choices));
        if !answered {
            skip_view();
        }
    }

    pub(in crate::project) fn palette_key(&mut self, key: Key, modifiers: Modifiers) -> bool {
        if modifiers != Modifiers::NONE {
            return false;
        }
        let Some(palette) = self.palette.as_mut() else {
            return false;
        };
        match key {
            Key::ArrowUp => palette.step(-1),
            Key::ArrowDown => palette.step(1),
            Key::PageUp => palette.page(false),
            Key::PageDown => palette.page(true),
            Key::Escape => *self.palette = None,
            _ => return false,
        }
        true
    }

    pub(in crate::project) fn take_palette(&mut self, at: Option<usize>) {
        let Some(palette) = self.palette.as_mut() else {
            skip_view();
            return;
        };
        let at = at.unwrap_or(palette.chosen());
        let Some(choice) = palette.choose(at).cloned() else {
            skip_view();
            return;
        };
        *self.palette = None;
        let root = self.shown_root();
        let path = root.join(choice.path_match.path());
        self.turn_to(Tab::Files);
        self.explore(|explorer, mut parts| {
            parts.reveal(&path);
            explorer.choose(&path);
            parts.viewer.open(path.clone(), true, parts.git);
        });
    }
}

fn relative(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .and_then(|relative| relative.to_str())
        .map(str::to_owned)
}
