use std::ops::Range;

use crate::anchor::{Anchor, starts_with_anchor};
use crate::edit::Edit;
use crate::error::{Error, Result};
use crate::excerpt::Excerpt;
use crate::text::{cut_line, lines_of};

const STALE_CONTEXT: usize = 2;
const TAIL_LINES: usize = 3;
const MAX_QUOTED: usize = 120;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Site {
    Lines,
    After,
    Before,
}

#[derive(Debug)]
pub(crate) struct Placement<'a> {
    pub(crate) edit: usize,
    pub(crate) site: Site,
    pub(crate) old: Range<usize>,
    pub(crate) lines: Vec<&'a str>,
}

pub(crate) fn place<'a>(
    path: &str,
    edits: &'a [Edit],
    before: &[&str],
) -> Result<Vec<Placement<'a>>> {
    check_anchors(path, edits, before)?;
    let mut placements = edits
        .iter()
        .enumerate()
        .map(|(index, edit)| placement(path, index, edits.len(), edit))
        .collect::<Result<Vec<Placement<'a>>>>()?;
    placements.sort_by_key(|placement| (placement.old.start, placement.old.end, placement.edit));
    check_overlaps(path, &placements)?;
    Ok(placements)
}

fn check_anchors(path: &str, edits: &[Edit], before: &[&str]) -> Result<()> {
    let mut stale: Vec<Anchor> = Vec::new();
    for anchor in edits.iter().flat_map(Edit::anchors) {
        if !anchor.matches(before) && !stale.contains(&anchor) {
            stale.push(anchor);
        }
    }
    if stale.is_empty() {
        return Ok(());
    }

    let count = before.len();
    let mut report: Vec<String> = stale
        .iter()
        .map(|anchor| match anchor.line() {
            _ if !anchor.is_tagged() => format!("`{anchor}` has no tag."),
            line if line > count => {
                format!("`{anchor}` is past the end; the file has {count} lines.")
            }
            line => format!("`{anchor}` is not line {line} as it reads now."),
        })
        .collect();
    let spans: Vec<Range<usize>> = stale
        .iter()
        .map(|anchor| match anchor.line().min(count) {
            line if line == anchor.line() => {
                line.saturating_sub(1 + STALE_CONTEXT)..(line + STALE_CONTEXT).min(count)
            }
            line => line.saturating_sub(TAIL_LINES)..line,
        })
        .collect();
    report.push(format!(
        "Those lines read now:\n{}",
        Excerpt::current(before, &spans)
    ));
    report.push("Take anchors from these lines, or from a fresh numbered listing.".to_owned());
    Err(Error::StaleAnchor {
        path: path.to_owned(),
        report: report.join("\n"),
    })
}

fn placement<'a>(path: &str, index: usize, count: usize, edit: &'a Edit) -> Result<Placement<'a>> {
    let lines = lines_of(edit.text());
    if let Some((at, line)) = lines
        .iter()
        .enumerate()
        .find(|(_, line)| starts_with_anchor(line))
    {
        return Err(Error::ListingPrefix {
            path: path.to_owned(),
            report: format!(
                "{}text line {} keeps the anchor printed before it in the listing:\n{}\nThe text holds only the new lines, without numbers or tags.",
                owner(index, count),
                at + 1,
                cut_line(line, MAX_QUOTED)
            ),
        });
    }

    let (site, old) = match edit {
        Edit::Replace { first, last, .. } if last.line() < first.line() => {
            return Err(Error::ReversedRange {
                path: path.to_owned(),
                report: format!(
                    "{}range ends at `{last}`, before it starts at `{first}`.",
                    owner(index, count)
                ),
            });
        }
        Edit::Replace { first, last, .. } => (Site::Lines, first.line() - 1..last.line()),
        Edit::InsertAfter { anchor, .. } => (Site::After, anchor.line()..anchor.line()),
        Edit::InsertBefore { anchor, .. } => (Site::Before, anchor.line() - 1..anchor.line() - 1),
    };
    Ok(Placement {
        edit: index,
        site,
        old,
        lines,
    })
}

fn check_overlaps(path: &str, placements: &[Placement<'_>]) -> Result<()> {
    let mut reached: Option<(usize, usize)> = None;
    for placement in placements {
        if let Some((_, owner)) = reached.filter(|(end, _)| placement.old.start < *end) {
            let (one, two) = (owner.min(placement.edit) + 1, owner.max(placement.edit) + 1);
            let line = placement.old.start + usize::from(!placement.old.is_empty());
            return Err(Error::Overlap {
                path: path.to_owned(),
                report: format!(
                    "Edits {one} and {two} both reach line {line}. Anchors in one call refer to the file before it, so make them one edit."
                ),
            });
        }
        if !placement.old.is_empty() && reached.is_none_or(|(end, _)| placement.old.end > end) {
            reached = Some((placement.old.end, placement.edit));
        }
    }
    Ok(())
}

pub(crate) fn owner(index: usize, count: usize) -> String {
    match count {
        1 => "The edit's ".to_owned(),
        _ => format!("Edit {}'s ", index + 1),
    }
}
