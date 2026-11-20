use std::ops::Range;

use rikka_theme::Token;
use tree_sitter as ts;
use tree_sitter::StreamingIterator;

use crate::grammar::Rules;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Region {
    pub(crate) range: Range<usize>,
    pub(crate) token: Token,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RawRegion {
    start: usize,
    end: usize,
    depth: u8,
    token: Token,
}

pub(crate) fn gather<'tree, I, T>(
    rules: &Rules,
    node: ts::Node<'tree>,
    text: T,
    within: &Range<usize>,
    depth: u8,
    into: &mut Vec<RawRegion>,
) where
    I: AsRef<[u8]>,
    T: ts::TextProvider<I>,
{
    let mut cursor = ts::QueryCursor::new();
    cursor.set_byte_range(within.clone());
    let mut captures = cursor.captures(&rules.highlights, node, text);
    let mut pending: Option<(usize, Option<RawRegion>)> = None;
    while let Some((query_match, slot)) = captures.next() {
        let capture = query_match.captures()[*slot];
        let range = capture.node.byte_range();
        if range.is_empty() {
            continue;
        }
        let capture_region = rules.tokens[capture.index as usize].map(|token| RawRegion {
            start: range.start,
            end: range.end,
            depth,
            token,
        });
        match &mut pending {
            Some((node, captured)) if *node == capture.node.id() => *captured = capture_region,
            _ => {
                if let Some((_, Some(captured))) =
                    pending.replace((capture.node.id(), capture_region))
                {
                    into.push(captured);
                }
            }
        }
    }
    if let Some((_, Some(captured))) = pending {
        into.push(captured);
    }
}

pub(crate) fn flatten(mut captured: Vec<RawRegion>, within: Range<usize>, into: &mut Vec<Region>) {
    captured.sort_by(|one, other| {
        one.start
            .cmp(&other.start)
            .then(other.end.cmp(&one.end))
            .then(one.depth.cmp(&other.depth))
    });

    let mut open: Vec<(usize, Token)> = Vec::new();
    let mut at = within.start;
    for entry in captured {
        let start = entry.start.clamp(within.start, within.end);
        let end = entry.end.clamp(within.start, within.end);
        if start >= end {
            continue;
        }
        while let Some(&(closes, token)) = open.last() {
            if closes > start {
                break;
            }
            paint(into, at..closes, token);
            at = at.max(closes);
            open.pop();
        }
        if let Some(&(_, token)) = open.last() {
            paint(into, at..start, token);
        }
        at = at.max(start);
        open.push((end, entry.token));
    }
    while let Some((closes, token)) = open.pop() {
        paint(into, at..closes, token);
        at = at.max(closes);
    }
}

fn paint(into: &mut Vec<Region>, range: Range<usize>, token: Token) {
    if range.start >= range.end {
        return;
    }
    if let Some(last) = into.last_mut()
        && last.token == token
        && last.range.end == range.start
    {
        last.range.end = range.end;
        return;
    }
    into.push(Region { range, token });
}
