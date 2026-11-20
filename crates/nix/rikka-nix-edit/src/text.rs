use std::ops::Range;

pub(crate) fn line_start(source: &str, at: usize) -> usize {
    source[..at].rfind('\n').map_or(0, |index| index + 1)
}

fn line_end(source: &str, at: usize) -> usize {
    source[at..]
        .find('\n')
        .map_or(source.len(), |index| at + index)
}

pub(crate) fn indent(source: &str, at: usize) -> usize {
    let start = line_start(source, at);
    source[start..].chars().take_while(|ch| *ch == ' ').count()
}

pub(crate) fn heads_line(source: &str, at: usize) -> bool {
    source[line_start(source, at)..at].trim().is_empty()
}

pub(crate) fn spaces(count: usize) -> String {
    " ".repeat(count)
}

pub(crate) struct Removal {
    pub(crate) range: Range<usize>,
    pub(crate) lines: bool,
}

pub(crate) fn removal(source: &str, entry: Range<usize>) -> Removal {
    let Range { mut start, mut end } = entry;
    let after = line_end(source, end);
    let rest = source[end..after].trim_start_matches([' ', '\t']);
    let clears_line = rest.is_empty() || rest.starts_with('#');
    if !clears_line {
        end = after - rest.len();
        return Removal {
            range: start..end,
            lines: false,
        };
    }
    end = after;
    if !heads_line(source, start) {
        start = source[..start].trim_end_matches([' ', '\t']).len();
        return Removal {
            range: start..end,
            lines: false,
        };
    }
    start = line_start(source, start);
    if end < source.len() {
        end += 1;
    }
    Removal {
        range: start..end,
        lines: true,
    }
}
pub(crate) fn joined(source: &str, mut spans: Vec<Range<usize>>) -> Vec<Range<usize>> {
    spans.sort_by_key(|span| span.start);
    let mut joined: Vec<Range<usize>> = Vec::with_capacity(spans.len());
    for span in spans {
        match joined.last_mut() {
            Some(last)
                if source[last.end..span.start]
                    .trim_matches([' ', '\t'])
                    .is_empty() =>
            {
                last.end = span.end;
            }
            _ => joined.push(span),
        }
    }
    joined
}

pub(crate) fn tidy(source: &mut String, at: usize) {
    let next = source[at..].lines().next().unwrap_or("").trim();
    if at > 0 {
        let above = line_start(source, at - 1);
        let previous = source[above..at - 1].trim();
        if previous.is_empty() && above > 0 && (next.is_empty() || next.starts_with('}')) {
            source.replace_range(above..at, "");
            return;
        }
        if previous.ends_with('{') && next.is_empty() && at < source.len() {
            let below = line_end(source, at);
            source.replace_range(at..(below + 1).min(source.len()), "");
        }
    }
}
