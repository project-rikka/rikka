use std::ops::Range;

use rikka_syntax::Comment;
use rikka_ui::text::Content;

pub(crate) fn toggle(content: &mut Content, comment: Comment) -> bool {
    let edits = match comment {
        Comment::Line(starts) => lines(content, starts),
        Comment::Block(opens, closes) => block(content, opens, closes),
    };
    if edits.is_empty() {
        return false;
    }
    let (anchor, caret) = (content.anchor(), content.caret());
    let changed = content.replace_each(&edits);
    content.select(moved(anchor, &edits), moved(caret, &edits));
    changed
}

fn covered(content: &Content) -> Range<usize> {
    let selection = content.selection();
    let first = content.line_of(selection.start);
    let mut last = content.line_of(selection.end);
    if last > first && content.line_start(last) == selection.end {
        last -= 1;
    }
    first..last + 1
}

fn text_of(content: &Content, line: usize) -> String {
    let range = content.line_range(line);
    content
        .slice(range)
        .trim_end_matches(['\n', '\r'])
        .to_owned()
}

fn lines(content: &Content, starts: &str) -> Vec<(Range<usize>, String)> {
    let lines: Vec<(usize, String)> = covered(content)
        .map(|line| (content.line_start(line), text_of(content, line)))
        .collect();
    let filled: Vec<&(usize, String)> = lines
        .iter()
        .filter(|(_, text)| !text.trim().is_empty())
        .collect();
    let commented = !filled.is_empty()
        && filled
            .iter()
            .all(|(_, text)| text.trim_start().starts_with(starts));
    if commented {
        return filled
            .iter()
            .map(|(start, text)| {
                let indent = text.len() - text.trim_start().len();
                let rest = &text[indent + starts.len()..];
                let spaced = usize::from(rest.starts_with(' '));
                let at = start + indent;
                (at..at + starts.len() + spaced, String::new())
            })
            .collect();
    }
    let targets: Vec<&(usize, String)> = match filled.is_empty() {
        true => lines.iter().take(1).collect(),
        false => filled,
    };
    let column = targets
        .iter()
        .map(|(_, text)| text.len() - text.trim_start().len())
        .min()
        .unwrap_or(0);
    targets
        .iter()
        .map(|(start, _)| {
            let at = start + column;
            (at..at, format!("{starts} "))
        })
        .collect()
}

fn block(content: &Content, opens: &str, closes: &str) -> Vec<(Range<usize>, String)> {
    let selection = content.selection();
    let range = match selection.is_empty() {
        true => {
            let line = content.line_of(selection.start);
            let start = content.line_start(line);
            let text = text_of(content, line);
            let indent = text.len() - text.trim_start().len();
            start + indent..start + text.trim_end().len()
        }
        false => selection,
    };
    let text = content.slice(range.clone());
    let trimmed_start = text.len() - text.trim_start().len();
    let inner = text.trim();
    if inner.starts_with(opens)
        && inner.ends_with(closes)
        && inner.len() >= opens.len() + closes.len()
    {
        let open_at = range.start + trimmed_start;
        let close_at = open_at + inner.len() - closes.len();
        let body = &inner[opens.len()..inner.len() - closes.len()];
        let open_end = open_at + opens.len() + usize::from(body.starts_with(' '));
        let close_start = close_at - usize::from(body.ends_with(' ') && body.len() > 1);
        return vec![
            (open_at..open_end, String::new()),
            (close_start..close_at + closes.len(), String::new()),
        ];
    }
    vec![
        (range.start..range.start, format!("{opens} ")),
        (range.end..range.end, format!(" {closes}")),
    ]
}

fn moved(at: usize, edits: &[(Range<usize>, String)]) -> usize {
    let mut shift: isize = 0;
    for (range, replacement) in edits {
        if range.start >= at {
            break;
        }
        if range.end > at {
            return (range.start as isize + shift).max(0) as usize;
        }
        shift += replacement.len() as isize - range.len() as isize;
    }
    (at as isize + shift).max(0) as usize
}
