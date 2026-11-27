use std::ops::Range;

use crate::change::Change;
use crate::diff::{self, DiffHunk};
use crate::excerpt::Excerpt;

#[derive(Debug)]
pub(crate) struct Edited<'a> {
    pub(crate) path: &'a str,
    pub(crate) before: Vec<&'a str>,
    pub(crate) after: Vec<&'a str>,
    hunks: Vec<DiffHunk>,
}

impl<'a> Edited<'a> {
    pub(crate) fn between(path: &'a str, before: Vec<&'a str>, after: Vec<&'a str>) -> Edited<'a> {
        let hunks = diff::changes(&before, &after);
        Edited {
            path,
            before,
            after,
            hunks,
        }
    }

    pub(crate) fn change(&self) -> Change {
        Change::from_edits(self.path.to_owned(), &self.before, &self.after, &self.hunks)
    }

    pub(crate) fn excerpt_after(&self) -> Excerpt {
        let spans: Vec<Range<usize>> = self.hunks.iter().map(|hunk| hunk.side.clone()).collect();
        Excerpt::around(&self.after, &spans)
    }

    pub(crate) fn excerpt_before(&self) -> Excerpt {
        let spans: Vec<Range<usize>> = self.hunks.iter().map(|hunk| hunk.base.clone()).collect();
        Excerpt::around(&self.before, &spans)
    }

    pub(crate) fn shifted_line(&self, line: usize) -> Option<usize> {
        let mut shift = 0isize;
        for hunk in &self.hunks {
            if line < hunk.base.start {
                break;
            }
            if hunk.base.contains(&line) {
                return None;
            }
            shift += hunk.side.len() as isize - hunk.base.len() as isize;
        }
        line.checked_add_signed(shift)
    }
}
