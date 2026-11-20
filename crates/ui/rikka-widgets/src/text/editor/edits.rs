use std::ops::Range;
use std::sync::Arc;

use rikka_theme::Palette;
use rikka_types::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit {
    Added,
    Changed,
    Gone,
}

impl Edit {
    pub(crate) fn color(self, palette: Palette) -> Color {
        match self {
            Edit::Added => palette.success,
            Edit::Changed => palette.warning,
            Edit::Gone => palette.danger,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Edits {
    lines: Arc<[(usize, Edit)]>,
}

impl Edits {
    #[must_use]
    pub fn new(lines: impl IntoIterator<Item = (usize, Edit)>) -> Edits {
        let mut lines: Vec<(usize, Edit)> = lines.into_iter().collect();
        lines.sort_by_key(|(line, _)| *line);
        lines.dedup_by_key(|(line, _)| *line);
        Edits {
            lines: lines.into(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub(crate) fn in_lines(&self, lines: Range<usize>) -> &[(usize, Edit)] {
        let start = self.lines.partition_point(|(line, _)| *line < lines.start);
        let end = self.lines.partition_point(|(line, _)| *line < lines.end);
        &self.lines[start..end]
    }
}
