use std::ops::Range;

use crate::change::Change;
use crate::diff::{self, DiffHunk};
use crate::eol::Returns;
use crate::excerpt::Excerpt;
use crate::placement::{Placement, Site};

#[derive(Debug)]
pub(crate) struct Splice {
    pub(crate) edit: usize,
    pub(crate) site: Site,
    pub(crate) old: Range<usize>,
    pub(crate) new: Range<usize>,
}

impl Splice {
    pub(crate) fn is_insertion(&self) -> bool {
        self.old.is_empty()
    }
}

#[derive(Debug)]
pub(crate) struct Edited<'a> {
    pub(crate) path: &'a str,
    pub(crate) before: Vec<&'a str>,
    pub(crate) after: Vec<&'a str>,
    pub(crate) splices: Vec<Splice>,
    pub(crate) edits: usize,
    returns: Returns,
    hunks: Vec<DiffHunk>,
}

impl<'a> Edited<'a> {
    pub(crate) fn apply(
        path: &'a str,
        before: Vec<&'a str>,
        returns: &Returns,
        placements: Vec<Placement<'a>>,
        edits: usize,
    ) -> Edited<'a> {
        let mut after = Vec::with_capacity(before.len());
        let mut after_returns = Returns::default();
        let mut splices = Vec::with_capacity(placements.len());
        let mut at = 0;

        for placement in placements {
            after.extend_from_slice(&before[at..placement.old.start]);
            after_returns.extend_from(returns, at..placement.old.start);
            let start = after.len();
            after_returns.extend_replacing(
                returns,
                &placement.old,
                placement.site,
                placement.lines.len(),
            );
            after.extend(placement.lines);
            splices.push(Splice {
                edit: placement.edit,
                site: placement.site,
                old: placement.old.clone(),
                new: start..after.len(),
            });
            at = placement.old.end;
        }
        after.extend_from_slice(&before[at..]);
        after_returns.extend_from(returns, at..before.len());

        let hunks = diff::changes(&before, &after);
        Edited {
            path,
            before,
            after,
            splices,
            edits,
            returns: after_returns,
            hunks,
        }
    }

    pub(crate) fn is_unchanged(&self) -> bool {
        self.hunks.is_empty()
    }

    pub(crate) fn content(&self, newline: bool) -> String {
        self.returns.join(&self.after, newline)
    }

    pub(crate) fn change(&self) -> Change {
        Change::from_edits(self.path.to_owned(), &self.before, &self.after, &self.hunks)
    }

    pub(crate) fn excerpt_written(&self) -> Excerpt {
        Excerpt::current(&self.after, &self.after_spans())
    }

    pub(crate) fn excerpt_proposed(&self) -> Excerpt {
        Excerpt::proposed(&self.after, &self.after_spans())
    }

    pub(crate) fn excerpt_before(&self) -> Excerpt {
        let spans: Vec<Range<usize>> = self.hunks.iter().map(|hunk| hunk.base.clone()).collect();
        Excerpt::current(&self.before, &spans)
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

    fn after_spans(&self) -> Vec<Range<usize>> {
        self.hunks.iter().map(|hunk| hunk.side.clone()).collect()
    }
}
