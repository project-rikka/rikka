use std::ops::Range;

use crate::matching::line_of;

#[derive(Debug)]
pub(crate) enum Edge {
    Above,
    Below,
}

#[derive(Debug)]
pub(crate) struct Echo {
    pub(crate) edge: Edge,
    pub(crate) line: usize,
    pub(crate) replaced: Range<usize>,
    pub(crate) neighbour: Range<usize>,
}

pub(crate) fn echo(content: &str, at: Range<usize>, new: &str) -> Option<Echo> {
    let old = &content[at.clone()];
    let bytes = content.as_bytes();
    let starts_line = at.start == 0 || bytes[at.start - 1] == b'\n';
    let ends_line = old.ends_with('\n') || bytes.get(at.end).is_none_or(|&byte| byte == b'\n');
    if !starts_line || !ends_line {
        return None;
    }

    let (old_lines, new_lines) = (lines(old), lines(new));
    let (first, last) = (new_lines.first()?, new_lines.last()?);
    let above = line_before(content, at.start)
        .filter(|line| is_echo(&content[line.clone()], first) && !same_trimmed(old_lines[0], first))
        .map(|line| (Edge::Above, line));
    let (edge, neighbour) = above.or_else(|| {
        let from = at.end + usize::from(!old.ends_with('\n'));
        line_from(content, from)
            .filter(|line| {
                is_echo(&content[line.clone()], last)
                    && !same_trimmed(old_lines[old_lines.len() - 1], last)
            })
            .map(|line| (Edge::Below, line))
    })?;

    let start = line_of(content, at.start);
    let replaced = start..start + old_lines.len();
    let line = match edge {
        Edge::Above => start,
        Edge::Below => replaced.end + 1,
    };
    Some(Echo {
        edge,
        line,
        replaced,
        neighbour,
    })
}

fn line_before(content: &str, start: usize) -> Option<Range<usize>> {
    let end = start.checked_sub(1)?;
    let begin = content[..end].rfind('\n').map_or(0, |at| at + 1);
    Some(begin..end)
}

fn line_from(content: &str, from: usize) -> Option<Range<usize>> {
    let rest = content.get(from..).filter(|rest| !rest.is_empty())?;
    Some(from..from + rest.find('\n').unwrap_or(rest.len()))
}

fn lines(text: &str) -> Vec<&str> {
    match text.strip_suffix('\n').unwrap_or(text) {
        "" if text.is_empty() => Vec::new(),
        body => body.split('\n').collect(),
    }
}

fn same_trimmed(one: &str, other: &str) -> bool {
    one.trim() == other.trim()
}

fn is_echo(unchanged: &str, written: &str) -> bool {
    same_trimmed(unchanged, written) && unchanged.chars().any(char::is_alphanumeric)
}
