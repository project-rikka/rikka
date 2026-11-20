use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

const CONTEXT_LINES: usize = 2;
const EDGE_LINES: usize = 2;
const MAX_FULL_SPAN: usize = 6;
const MAX_LINES: usize = 160;
const MAX_LINE_CHARS: usize = 240;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Row {
    Line { number: usize, text: String },
    Skipped,
    Truncated,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Excerpt {
    rows: Vec<Row>,
}

impl Excerpt {
    pub(crate) fn around(lines: &[&str], spans: &[Range<usize>]) -> Excerpt {
        let excerpt_after = excerpt_after(spans, lines.len());
        let mut rows = Vec::new();
        let mut last: Option<usize> = None;

        for &at in &excerpt_after {
            let skips = last.is_some_and(|last| at > last + 1);
            if rows.len() + usize::from(skips) >= MAX_LINES {
                rows.push(Row::Truncated);
                break;
            }
            if skips {
                rows.push(Row::Skipped);
            }
            rows.push(Row::Line {
                number: at + 1,
                text: truncate_line(lines[at]),
            });
            last = Some(at);
        }

        Excerpt { rows }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

impl fmt::Display for Excerpt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, row) in self.rows.iter().enumerate() {
            if index > 0 {
                formatter.write_str("\n")?;
            }
            match row {
                Row::Line { number, text } => write!(formatter, "{number:>6}\t{text}")?,
                Row::Skipped => write!(formatter, "{:>6}", "…")?,
                Row::Truncated => write!(formatter, "{:>6}\tthe rest is not shown", "…")?,
            }
        }
        Ok(())
    }
}

fn excerpt_after(spans: &[Range<usize>], count: usize) -> BTreeSet<usize> {
    let mut excerpt_after = BTreeSet::new();
    for span in spans {
        let (start, end) = (span.start.min(count), span.end.min(count));
        excerpt_after.extend(start.saturating_sub(CONTEXT_LINES)..start);
        match end - start <= MAX_FULL_SPAN {
            true => excerpt_after.extend(start..end),
            false => {
                excerpt_after.extend(start..start + EDGE_LINES);
                excerpt_after.extend(end - EDGE_LINES..end);
            }
        }
        excerpt_after.extend(end..(end + CONTEXT_LINES).min(count));
    }
    excerpt_after
}

fn truncate_line(line: &str) -> String {
    match line.char_indices().nth(MAX_LINE_CHARS) {
        Some((cut, _)) => format!("{}…", &line[..cut]),
        None => line.to_owned(),
    }
}
