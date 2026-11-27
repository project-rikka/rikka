use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;

use crate::anchor::Tag;
use crate::text::cut_line;

const CONTEXT_LINES: usize = 2;
const EDGE_LINES: usize = 2;
const MAX_FULL_SPAN: usize = 6;
const MAX_LINES: usize = 160;
const MAX_LINE_CHARS: usize = 2000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Numbering {
    Anchored,
    Plain,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Row {
    Line {
        number: usize,
        tag: Option<Tag>,
        text: String,
    },
    Skipped,
    Truncated,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Excerpt {
    rows: Vec<Row>,
}

impl Excerpt {
    pub(crate) fn current(lines: &[&str], spans: &[Range<usize>]) -> Excerpt {
        Excerpt::around(lines, spans, Numbering::Anchored)
    }

    pub(crate) fn proposed(lines: &[&str], spans: &[Range<usize>]) -> Excerpt {
        Excerpt::around(lines, spans, Numbering::Plain)
    }

    fn around(lines: &[&str], spans: &[Range<usize>], numbering: Numbering) -> Excerpt {
        let shown = shown(spans, lines.len());
        let mut rows = Vec::new();
        let mut last: Option<usize> = None;

        for &at in &shown {
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
                tag: (numbering == Numbering::Anchored).then(|| Tag::of(at + 1, lines[at])),
                text: cut_line(lines[at], MAX_LINE_CHARS).into_owned(),
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
                Row::Line { number, tag, text } => {
                    write!(formatter, "{number:>6}")?;
                    if let Some(tag) = tag {
                        write!(formatter, "#{tag}")?;
                    }
                    write!(formatter, "\t{text}")?;
                }
                Row::Skipped => write!(formatter, "{:>6}", "…")?,
                Row::Truncated => write!(formatter, "{:>6}\tthe rest is not shown", "…")?,
            }
        }
        Ok(())
    }
}

fn shown(spans: &[Range<usize>], count: usize) -> BTreeSet<usize> {
    let mut shown = BTreeSet::new();
    for span in spans {
        let (start, end) = (span.start.min(count), span.end.min(count));
        shown.extend(start.saturating_sub(CONTEXT_LINES)..start);
        match end - start <= MAX_FULL_SPAN {
            true => shown.extend(start..end),
            false => {
                shown.extend(start..start + EDGE_LINES);
                shown.extend(end - EDGE_LINES..end);
            }
        }
        shown.extend(end..(end + CONTEXT_LINES).min(count));
    }
    shown
}
