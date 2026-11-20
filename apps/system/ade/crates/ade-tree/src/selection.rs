use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::rebase::rebased;
use crate::row::Row;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    chosen: BTreeSet<PathBuf>,
    focus: Option<PathBuf>,
    anchor: Option<PathBuf>,
}

impl Selection {
    pub fn only(&mut self, path: &Path) {
        self.chosen.clear();
        self.chosen.insert(path.to_owned());
        self.focus = Some(path.to_owned());
        self.anchor = Some(path.to_owned());
    }

    pub fn toggle(&mut self, path: &Path) {
        if !self.chosen.remove(path) {
            self.chosen.insert(path.to_owned());
        }
        self.focus = Some(path.to_owned());
        self.anchor = Some(path.to_owned());
    }

    pub fn extend(&mut self, to: &Path, rows: &[Row], adding: bool) {
        let anchor = self.anchor.clone().unwrap_or_else(|| to.to_owned());
        let at = |path: &Path| rows.iter().position(|row| row.path() == path);
        let (Some(from), Some(until)) = (at(&anchor), at(to)) else {
            self.only(to);
            return;
        };
        if !adding {
            self.chosen.clear();
        }
        let span = from.min(until)..=from.max(until);
        self.chosen
            .extend(rows[span].iter().map(|row| row.path().clone()));
        self.focus = Some(to.to_owned());
        self.anchor = Some(anchor);
    }

    pub fn clear(&mut self) {
        *self = Selection::default();
    }

    #[must_use]
    pub fn contains(&self, path: &Path) -> bool {
        self.chosen.contains(path)
    }

    #[must_use]
    pub fn focus(&self) -> Option<&Path> {
        self.focus.as_deref()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.chosen.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.chosen.len()
    }

    #[must_use]
    pub fn topmost(&self) -> Vec<PathBuf> {
        self.chosen
            .iter()
            .filter(|path| {
                !path
                    .ancestors()
                    .skip(1)
                    .any(|above| self.chosen.contains(above))
            })
            .cloned()
            .collect()
    }

    pub fn keep(&mut self, exists: impl Fn(&Path) -> bool) {
        self.chosen.retain(|path| exists(path));
        if self.focus.as_deref().is_some_and(|focus| !exists(focus)) {
            self.focus = None;
        }
        if self.anchor.as_deref().is_some_and(|anchor| !exists(anchor)) {
            self.anchor = self.focus.clone();
        }
    }

    pub fn moved(&mut self, from: &Path, to: &Path) {
        let shift = |path: &PathBuf| rebased(path, from, to).unwrap_or_else(|| path.clone());
        self.chosen = self.chosen.iter().map(shift).collect();
        self.focus = self.focus.as_ref().map(shift);
        self.anchor = self.anchor.as_ref().map(shift);
    }
}
