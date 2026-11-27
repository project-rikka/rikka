use serde::{Deserialize, Serialize};

use crate::diff;

const CONTEXT_LINES: usize = 3;

const MAX_LINES: usize = 400;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Line {
    #[serde(rename = "kept")]
    Context(String),
    Added(String),
    Removed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hunk {
    #[serde(rename = "at")]
    pub start_line: usize,
    pub lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Change {
    pub path: String,
    pub added: usize,
    pub removed: usize,
    pub hunks: Vec<Hunk>,
}

impl Change {
    pub(crate) fn from_edits(
        path: String,
        before: &[&str],
        after: &[&str],
        edits: &[diff::DiffHunk],
    ) -> Change {
        Change {
            path,
            added: edits.iter().map(|edit| edit.side.len()).sum(),
            removed: edits.iter().map(|edit| edit.base.len()).sum(),
            hunks: hunks(before, after, edits),
        }
    }
}

fn hunks(before: &[&str], after: &[&str], edits: &[diff::DiffHunk]) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut group: Vec<&diff::DiffHunk> = Vec::new();
    let mut budget = MAX_LINES;

    for edit in edits {
        if budget == 0 {
            break;
        }
        let sort_disjoint = group
            .last()
            .is_some_and(|last| edit.base.start > last.base.end + CONTEXT_LINES * 2);
        if sort_disjoint {
            hunks.push(build_hunk(before, after, &group, &mut budget));
            group.clear();
        }
        group.push(edit);
    }

    if !group.is_empty() && budget > 0 {
        hunks.push(build_hunk(before, after, &group, &mut budget));
    }
    hunks
}

fn build_hunk(
    before: &[&str],
    after: &[&str],
    group: &[&diff::DiffHunk],
    budget: &mut usize,
) -> Hunk {
    let (first, last) = (group[0], group[group.len() - 1]);
    let from = first.base.start.saturating_sub(CONTEXT_LINES);
    let to = (last.base.end + CONTEXT_LINES).min(before.len());

    let mut lines: Vec<Line> = Vec::new();
    let mut take = |taken: &mut dyn Iterator<Item = Line>| {
        lines.extend(taken.take(budget.saturating_sub(lines.len())));
    };

    take(&mut context(&before[from..first.base.start]));
    for (index, edit) in group.iter().enumerate() {
        if let Some(previous) = index.checked_sub(1) {
            take(&mut context(
                &before[group[previous].base.end..edit.base.start],
            ));
        }
        take(
            &mut before[edit.base.clone()]
                .iter()
                .map(|line| Line::Removed((*line).to_owned())),
        );
        take(
            &mut after[edit.side.clone()]
                .iter()
                .map(|line| Line::Added((*line).to_owned())),
        );
    }
    take(&mut context(&before[last.base.end..to]));

    *budget -= lines.len();
    Hunk {
        start_line: first.side.start.saturating_sub(first.base.start - from) + 1,
        lines,
    }
}

fn context<'a>(lines: &'a [&str]) -> impl Iterator<Item = Line> + 'a {
    lines.iter().map(|line| Line::Context((*line).to_owned()))
}
