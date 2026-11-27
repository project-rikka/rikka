use std::ops::Range;

use imara_diff::{Algorithm, Diff, InternedInput};

const DIFFED: &str = include_str!("../prompts/diffed.md");

const CONTEXT_LINES: usize = 2;

struct Edit {
    before: Range<usize>,
    after: Range<usize>,
}

pub(crate) fn sections(conveyed: &[String], standing: &[String]) -> Vec<String> {
    let dropped: Vec<&str> = conveyed
        .iter()
        .filter(|block| !standing.contains(block))
        .map(String::as_str)
        .collect();
    if !dropped.is_empty() && standing.len() < conveyed.len() {
        return standing.to_vec();
    }
    standing
        .iter()
        .filter(|block| !conveyed.contains(block))
        .map(|block| {
            counterpart(&dropped, standing, block)
                .and_then(|earlier| diffed(earlier, block))
                .unwrap_or_else(|| block.clone())
        })
        .collect()
}

fn counterpart<'a>(dropped: &[&'a str], standing: &[String], block: &str) -> Option<&'a str> {
    let first = first_line(block)?;
    let unique_in_standing = standing
        .iter()
        .filter(|other| first_line(other) == Some(first))
        .count()
        == 1;
    let mut earlier = dropped
        .iter()
        .filter(|earlier| first_line(earlier) == Some(first));
    let found = earlier.next()?;
    (unique_in_standing && earlier.next().is_none()).then_some(*found)
}

fn first_line(block: &str) -> Option<&str> {
    block.lines().next().filter(|line| !line.trim().is_empty())
}

fn diffed(earlier: &str, block: &str) -> Option<String> {
    let before: Vec<&str> = earlier.split('\n').collect();
    let after: Vec<&str> = block.split('\n').collect();
    let edits = edits(&before, &after);

    let mut text = format!("{}\n\"{}\"", DIFFED.trim(), before[0]);
    for group in grouped(&edits) {
        let first = &group[0];
        let last = &group[group.len() - 1];
        let mut at = first.before.start.saturating_sub(CONTEXT_LINES);
        text.push_str("\n@@");
        for edit in group {
            push(&mut text, ' ', &before[at..edit.before.start]);
            push(&mut text, '-', &before[edit.before.clone()]);
            push(&mut text, '+', &after[edit.after.clone()]);
            at = edit.before.end;
        }
        let to = (last.before.end + CONTEXT_LINES).min(before.len());
        push(&mut text, ' ', &before[at..to]);
    }

    (text.len() * 2 < block.len()).then_some(text)
}

fn edits(before: &[&str], after: &[&str]) -> Vec<Edit> {
    let mut input: InternedInput<&str> = InternedInput::default();
    input.update_before(before.iter().copied());
    input.update_after(after.iter().copied());

    let mut diff = Diff::compute(Algorithm::Histogram, &input);
    diff.postprocess_lines(&input);

    diff.hunks()
        .map(|hunk| Edit {
            before: hunk.before.start as usize..hunk.before.end as usize,
            after: hunk.after.start as usize..hunk.after.end as usize,
        })
        .collect()
}

fn grouped(edits: &[Edit]) -> Vec<&[Edit]> {
    let mut groups = Vec::new();
    let mut start = 0;
    for index in 1..edits.len() {
        if edits[index].before.start > edits[index - 1].before.end + CONTEXT_LINES * 2 {
            groups.push(&edits[start..index]);
            start = index;
        }
    }
    if start < edits.len() {
        groups.push(&edits[start..]);
    }
    groups
}

fn push(text: &mut String, mark: char, lines: &[&str]) {
    for line in lines {
        text.push('\n');
        text.push(mark);
        text.push_str(line);
    }
}
