use std::ops::Range;

use crate::excerpt::Excerpt;
use crate::op::ResolvedOp;

#[derive(Debug)]
pub(crate) struct Splice {
    pub(crate) old: Range<usize>,
    pub(crate) new: Range<usize>,
}

impl Splice {
    pub(crate) fn is_insertion(&self) -> bool {
        self.old.is_empty()
    }

    pub(crate) fn describe(&self) -> String {
        match self.old.len() {
            1 => format!("line {}", self.old.start + 1),
            _ => format!("lines {}-{}", self.old.start + 1, self.old.end),
        }
    }
}

#[derive(Debug)]
pub(crate) struct Edited<'a> {
    pub(crate) path: &'a str,
    pub(crate) before: Vec<&'a str>,
    pub(crate) after: Vec<&'a str>,
    pub(crate) splices: Vec<Splice>,
}

impl<'a> Edited<'a> {
    pub(crate) fn apply(
        path: &'a str,
        before: Vec<&'a str>,
        resolved: &'a [ResolvedOp<'a>],
        order: &[usize],
    ) -> Edited<'a> {
        let mut after = Vec::new();
        let mut splices = Vec::with_capacity(order.len());
        let mut at = 0;

        for &index in order {
            let op = &resolved[index];
            after.extend_from_slice(&before[at..op.from - 1]);
            let start = after.len();
            after.extend_from_slice(&op.lines);
            splices.push(Splice {
                old: op.from - 1..op.to,
                new: start..after.len(),
            });
            at = op.to;
        }
        after.extend_from_slice(&before[at..]);

        Edited {
            path,
            before,
            after,
            splices,
        }
    }

    pub(crate) fn excerpt_after(&self, lines: &[&str]) -> Excerpt {
        let spans: Vec<Range<usize>> = self.splices.iter().map(|op| op.new.clone()).collect();
        Excerpt::around(lines, &spans)
    }

    pub(crate) fn excerpt_before(&self) -> Excerpt {
        let spans: Vec<Range<usize>> = self.splices.iter().map(|op| op.old.clone()).collect();
        Excerpt::around(&self.before, &spans)
    }

    pub(crate) fn shifted_line(&self, line: usize) -> Option<usize> {
        let mut shift = 0isize;
        for splice in &self.splices {
            if line < splice.old.start {
                break;
            }
            if splice.old.contains(&line) {
                return None;
            }
            shift += splice.new.len() as isize - splice.old.len() as isize;
        }
        line.checked_add_signed(shift)
    }
}
