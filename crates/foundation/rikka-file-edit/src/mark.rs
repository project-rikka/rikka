use crate::text::lines_of;

use crate::diff;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Added,
    Changed,
    Removed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    lines: Vec<(usize, Mark)>,
}

impl Marks {
    #[must_use]
    pub fn diff(before: &str, after: &str) -> Marks {
        let (before, after) = (lines_of(before), lines_of(after));
        let mut lines = Vec::new();

        for edit in diff::changes(&before, &after) {
            if edit.side.is_empty() {
                lines.push((edit.side.start + 1, Mark::Removed));
                continue;
            }
            let mark = match edit.base.is_empty() {
                true => Mark::Added,
                false => Mark::Changed,
            };
            lines.extend(edit.side.map(|line| (line + 1, mark)));
        }
        Marks { lines }
    }

    #[must_use]
    pub fn lines(&self) -> &[(usize, Mark)] {
        &self.lines
    }
}
