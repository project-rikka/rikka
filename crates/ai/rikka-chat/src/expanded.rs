use std::collections::HashSet;

#[derive(Clone, Debug, Default)]
pub(crate) struct Expanded {
    calls: HashSet<String>,
    summaries: HashSet<u64>,
}

impl Expanded {
    pub(crate) fn holds(&self, call: &str) -> bool {
        self.calls.contains(call)
    }

    pub(crate) fn toggle(&mut self, call: String) {
        if !self.calls.remove(&call) {
            self.calls.insert(call);
        }
    }

    pub(crate) fn holds_summary(&self, turn: u64) -> bool {
        self.summaries.contains(&turn)
    }

    pub(crate) fn toggle_summary(&mut self, turn: u64) {
        if !self.summaries.remove(&turn) {
            self.summaries.insert(turn);
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.calls.is_empty() && self.summaries.is_empty()
    }
}
