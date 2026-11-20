use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::change::Change;

#[derive(Clone, Debug, Default)]
pub struct Draft {
    summary: String,
    description: String,
    ticked: BTreeSet<PathBuf>,
    unticked: BTreeSet<PathBuf>,
}

impl Draft {
    #[must_use]
    pub fn new(summary: impl Into<String>) -> Draft {
        Draft {
            summary: summary.into(),
            ..Draft::default()
        }
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    #[must_use]
    pub fn with_changes<'a>(
        mut self,
        changes: impl IntoIterator<Item = (&'a Change, bool)>,
    ) -> Self {
        for (change, ticked) in changes {
            let side = match ticked {
                true => &mut self.ticked,
                false => &mut self.unticked,
            };
            side.extend(change.paths());
        }
        self
    }

    #[must_use]
    pub fn summary(&self) -> &str {
        self.summary.trim()
    }

    pub(crate) fn message(&self) -> Vec<String> {
        let description = self.description.trim();
        let mut message = vec!["-m".to_owned(), self.summary().to_owned()];
        if !description.is_empty() {
            message.push("-m".to_owned());
            message.push(description.to_owned());
        }
        message
    }

    pub(crate) fn staged(&self) -> Vec<PathBuf> {
        self.ticked.difference(&self.unticked).cloned().collect()
    }

    pub(crate) fn unstaged(&self) -> Vec<PathBuf> {
        self.unticked.iter().cloned().collect()
    }
}
