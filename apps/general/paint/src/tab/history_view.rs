use std::sync::Arc;

use paint_document::{Action, Document};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HistoryView {
    pub(crate) actions: Arc<[Action]>,
    pub(crate) current: usize,
    pub(crate) saved: Option<usize>,
}

impl HistoryView {
    pub(crate) fn from_document(document: &Document) -> HistoryView {
        let history = document.history();
        HistoryView {
            actions: Arc::from(history.actions()),
            current: history.current(),
            saved: history.saved(),
        }
    }

    pub(crate) fn can_undo(&self) -> bool {
        self.current > 0
    }

    pub(crate) fn can_redo(&self) -> bool {
        self.current + 1 < self.actions.len()
    }

    pub(crate) fn is_modified(&self) -> bool {
        self.saved != Some(self.current)
    }
}

impl Default for HistoryView {
    fn default() -> HistoryView {
        HistoryView {
            actions: Arc::from([Action::Open]),
            current: 0,
            saved: Some(0),
        }
    }
}
