use std::sync::Arc;

use ade_git::Patch;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Row {
    Divider,
    Heading { file: usize },
    Unreadable,
    Line { file: usize, at: usize },
}

#[derive(Debug, Default)]
pub(crate) struct Lines {
    rows: Vec<Row>,
}

impl Lines {
    pub(crate) fn from_patch(patch: &Patch, headed: bool) -> Lines {
        let mut rows = Vec::new();
        for (file, diff) in patch.files().iter().enumerate() {
            if headed {
                if file > 0 {
                    rows.push(Row::Divider);
                }
                rows.push(Row::Heading { file });
            }
            if diff.is_binary() {
                rows.push(Row::Unreadable);
                continue;
            }
            rows.extend((0..diff.lines().len()).map(|at| Row::Line { file, at }));
        }
        Lines { rows }
    }

    pub(crate) fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub(crate) fn at(&self, index: usize) -> Option<Row> {
        self.rows.get(index).copied()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Read {
    pub(crate) patch: Arc<Patch>,
    pub(crate) lines: Arc<Lines>,
}

impl Read {
    pub(crate) fn from_patch(patch: Patch, headed: bool) -> Read {
        let lines = Arc::new(Lines::from_patch(&patch, headed));
        Read {
            patch: Arc::new(patch),
            lines,
        }
    }
}
