use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Edit {
    Typing,
    Deleting,
    Appending,
    Other,
}

#[derive(Debug)]
pub(crate) struct History<T, const DEPTH: usize> {
    past: VecDeque<T>,
    future: Vec<T>,
    last: Option<Edit>,
}

impl<T, const DEPTH: usize> Default for History<T, DEPTH> {
    fn default() -> Self {
        Self {
            past: VecDeque::new(),
            future: Vec::new(),
            last: None,
        }
    }
}

impl<T: Clone, const DEPTH: usize> History<T, DEPTH> {
    pub(crate) fn record(&mut self, before: &T, kind: Edit) {
        if self.last != Some(kind) || kind == Edit::Other {
            if self.past.len() == DEPTH {
                self.past.pop_front();
            }
            self.past.push_back(before.clone());
        }
        self.last = Some(kind);
        self.future.clear();
    }

    pub(crate) fn interrupt(&mut self) {
        self.last = None;
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }

    pub(crate) fn undo(&mut self, current: &T) -> Option<T> {
        let previous = self.past.pop_back()?;
        self.future.push(current.clone());
        self.last = None;
        Some(previous)
    }

    pub(crate) fn redo(&mut self, current: &T) -> Option<T> {
        let next = self.future.pop()?;
        self.past.push_back(current.clone());
        self.last = None;
        Some(next)
    }

    pub(crate) fn clear(&mut self) {
        self.past.clear();
        self.future.clear();
        self.last = None;
    }
}
