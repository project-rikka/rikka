use installer_core::subsequence;

use crate::form::{Choice, FieldId};

#[derive(Debug)]
pub enum Overlay {
    Picker(Picker),
    Confirm(Confirm),
    Help,
}

#[derive(Debug)]
pub struct Picker {
    pub field: FieldId,
    pub title: String,
    pub query: String,
    options: Vec<Choice>,
    shown: Vec<usize>,
    cursor: usize,
}

impl Picker {
    #[must_use]
    pub fn new(
        field: FieldId,
        title: impl Into<String>,
        options: Vec<Choice>,
        selected: usize,
    ) -> Self {
        let shown = (0..options.len()).collect();
        Self {
            field,
            title: title.into(),
            query: String::new(),
            options,
            shown,
            cursor: selected,
        }
    }

    pub fn shown(&self) -> impl Iterator<Item = (usize, &Choice)> {
        self.shown
            .iter()
            .map(|index| (*index, &self.options[*index]))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.shown.len()
    }

    #[must_use]
    pub fn total(&self) -> usize {
        self.options.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shown.is_empty()
    }

    #[must_use]
    pub fn row(&self) -> usize {
        self.shown
            .iter()
            .position(|index| *index == self.cursor)
            .unwrap_or_default()
    }

    #[must_use]
    pub fn selected(&self) -> Option<usize> {
        self.shown.contains(&self.cursor).then_some(self.cursor)
    }

    pub fn move_by(&mut self, delta: isize) {
        if self.shown.is_empty() {
            return;
        }
        let len = self.shown.len() as isize;
        let row = (self.row() as isize + delta).clamp(0, len - 1);
        self.cursor = self.shown[row as usize];
    }

    pub fn push(&mut self, ch: char) {
        self.query.push(ch);
        self.refilter();
    }

    pub fn pop(&mut self) {
        self.query.pop();
        self.refilter();
    }

    fn refilter(&mut self) {
        self.shown = self
            .options
            .iter()
            .enumerate()
            .filter(|(_, choice)| subsequence(&choice.label, &self.query))
            .map(|(index, _)| index)
            .collect();
        if !self.shown.contains(&self.cursor) {
            self.cursor = self.shown.first().copied().unwrap_or_default();
        }
    }
}

#[derive(Debug)]
pub struct Confirm {
    pub title: String,
    pub body: Vec<String>,
    pub action: Decision,
    pub yes: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    BeginInstall,
    Quit,
}

impl Confirm {
    #[must_use]
    pub fn new(title: impl Into<String>, body: Vec<String>, action: Decision) -> Self {
        Self {
            title: title.into(),
            body,
            action,
            yes: false,
        }
    }
}
