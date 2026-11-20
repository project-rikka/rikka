use std::collections::BTreeSet;
use std::time::Duration;

use rikka_color_formats::{Grouping, Rgb};
use rikka_seam::Request;
use rikka_session::settings_app::Section;
use rikka_settings::session::color_picker::Format;
use rikka_types::{Color, Modifiers, Point};
use rikka_ui::{State, after_keyed, tr};
use shell_kit::launch::Spawn;
use shell_kit::{CLOSE, Spot, copy_to_clipboard};

use crate::CAPABILITY;
use crate::editor::export::{self, Exporting, Strings};
use crate::service::PICK;
use crate::shared::Shared;

const COPIED: &str = "color-picker.copied";

const COPIED_FOR: Duration = Duration::from_millis(1500);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MenuAnchor {
    pub(crate) index: usize,
    pub(crate) at: Point,
}

#[derive(State)]
pub(crate) struct Editor {
    shared: Shared,
    colors: Vec<Rgb>,
    focus: usize,
    anchor: usize,
    chosen: BTreeSet<usize>,
    menu: Option<MenuAnchor>,
    tuning: Option<Rgb>,
    copied: Option<usize>,
}

impl Editor {
    #[must_use]
    pub(crate) fn new(shared: Shared) -> Editor {
        let colors = shared.history.items();
        let chosen = match colors.is_empty() {
            true => BTreeSet::new(),
            false => BTreeSet::from([0]),
        };
        Editor {
            shared,
            colors,
            focus: 0,
            anchor: 0,
            chosen,
            menu: None,
            tuning: None,
            copied: None,
        }
    }

    #[must_use]
    pub(crate) fn spot(&self) -> &Spot {
        &self.shared.spot
    }

    #[must_use]
    pub(crate) fn colors(&self) -> &[Rgb] {
        &self.colors
    }

    #[must_use]
    pub(crate) fn current(&self) -> Option<Rgb> {
        self.colors.get(self.focus).copied()
    }

    #[must_use]
    pub(crate) fn focus(&self) -> usize {
        self.focus
    }

    #[must_use]
    pub(super) fn is_chosen(&self, index: usize) -> bool {
        self.chosen.contains(&index)
    }

    #[must_use]
    pub(crate) fn menu(&self) -> Option<MenuAnchor> {
        self.menu
    }

    #[must_use]
    pub(super) fn tuning(&self) -> Option<Rgb> {
        self.tuning
    }

    #[must_use]
    pub(crate) fn copied(&self) -> Option<usize> {
        self.copied
    }

    #[must_use]
    pub(crate) fn formats(&self) -> Vec<Format> {
        self.shared.preferences.shown()
    }

    pub(crate) fn focused(&mut self, index: usize, modifiers: Modifiers) {
        if index >= self.colors.len() {
            return;
        }
        if modifiers.shift {
            let (low, high) = (self.anchor.min(index), self.anchor.max(index));
            self.chosen = (low..=high).collect();
        } else if modifiers.control {
            if !self.chosen.remove(&index) || self.chosen.is_empty() {
                self.chosen.insert(index);
            }
            self.anchor = index;
        } else {
            self.chosen = BTreeSet::from([index]);
            self.anchor = index;
        }
        self.focus = index;
        self.tuning = None;
    }

    pub(crate) fn open_menu(&mut self, index: usize, at: Point) {
        if index >= self.colors.len() {
            return;
        }
        if !self.chosen.contains(&index) {
            self.chosen = BTreeSet::from([index]);
            self.anchor = index;
            self.focus = index;
        }
        self.menu = Some(MenuAnchor { index, at });
    }

    pub(super) fn dismiss_menu(&mut self) {
        self.menu = None;
    }

    pub(crate) fn remove(&mut self) {
        self.menu = None;
        if self.chosen.is_empty() {
            return;
        }
        let gone: Vec<Rgb> = self.picked();
        let last = self.chosen.last().copied().unwrap_or(0);
        let remaining: Vec<usize> = (0..self.colors.len())
            .filter(|index| !self.chosen.contains(index))
            .collect();
        let after = remaining
            .iter()
            .position(|index| *index > last)
            .or_else(|| remaining.len().checked_sub(1));
        self.shared.history.remove(&gone);
        self.colors = self.shared.history.items();
        self.focus = after.unwrap_or(0);
        self.anchor = self.focus;
        self.chosen = match self.colors.is_empty() {
            true => BTreeSet::new(),
            false => BTreeSet::from([self.focus]),
        };
    }

    pub(crate) fn adopt(&mut self, rgb: Rgb) {
        self.shared.history.record(rgb);
        self.colors = self.shared.history.items();
        self.focus = 0;
        self.anchor = 0;
        self.chosen = BTreeSet::from([0]);
        self.tuning = None;
    }

    pub(crate) fn copy(&mut self, row: usize, rendered_text: String) {
        copy_to_clipboard(&rendered_text);
        self.copied = Some(row);
        after_keyed(COPIED, COPIED_FOR, |editor: &mut Editor| {
            editor.copied = None
        });
    }

    pub(super) fn copy_current(&mut self) {
        if let Some(current) = self.current() {
            copy_to_clipboard(&self.shared.preferences.copied().render(current));
        }
    }

    pub(super) fn tune(&mut self) {
        self.tuning = self.current();
    }

    pub(super) fn tuned(&mut self, color: Color) {
        self.tuning = Some(Rgb::from(color));
    }

    pub(super) fn apply_tune(&mut self) {
        if let Some(tuned) = self.tuning.take() {
            self.adopt(tuned);
        }
    }

    pub(super) fn cancel_tune(&mut self) {
        self.tuning = None;
    }

    pub(crate) fn pick(&mut self) {
        self.shared.seam.notify(Request::new(CAPABILITY, PICK));
    }

    pub(crate) fn dismiss(&mut self) {
        self.shared.seam.notify(Request::new(CAPABILITY, CLOSE));
    }

    pub(crate) fn settings(&mut self) {
        self.shared
            .seam
            .notify(Spawn::settings(Section::ColorPicker).request());
        self.dismiss();
    }

    pub(super) fn export(&mut self, grouping: Grouping) {
        self.menu = None;
        let colors = self.picked();
        if colors.is_empty() {
            return;
        }
        let exporting = Exporting {
            colors,
            formats: self.formats(),
            grouping,
            words: Strings {
                app_name: tr!("shell.color-picker.name").to_string(),
                title: tr!("shell.color-picker.export.title").to_string(),
                stem: tr!("shell.color-picker.export.stem").to_string(),
                text: tr!("shell.color-picker.export.text").to_string(),
                json: tr!("shell.color-picker.export.json").to_string(),
                failed: tr!("shell.color-picker.export.failed").to_string(),
            },
            seam: self.shared.seam.clone(),
        };
        self.shared
            .editor
            .after_unmap(move || export::start(exporting));
        self.dismiss();
    }

    fn picked(&self) -> Vec<Rgb> {
        self.chosen
            .iter()
            .filter_map(|index| self.colors.get(*index).copied())
            .collect()
    }
}

impl std::fmt::Debug for Editor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field("colors", &self.colors.len())
            .field("focus", &self.focus)
            .finish_non_exhaustive()
    }
}
