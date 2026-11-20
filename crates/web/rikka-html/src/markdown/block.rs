use super::inline::{self, Setting};
use super::longest_backtick_run;
use super::phrasing::Phrasing;

pub(super) const LAST_NUMBER: u64 = 999_999_999;

#[derive(Debug)]
pub(super) enum Block {
    Paragraph(Phrasing),
    Heading(usize, Phrasing),
    Quote(Vec<Block>),
    List(List),
    Code(String),
    Rule,
    Table(Vec<Vec<Phrasing>>),
}

#[derive(Debug)]
pub(super) struct List {
    pub(super) start: Option<u64>,
    pub(super) items: Vec<Vec<Block>>,
}

pub(super) fn render(blocks: &[Block]) -> String {
    let mut out = String::new();
    let mut alternate = false;
    let mut previous: Option<&Block> = None;
    for block in blocks {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        match block {
            Block::List(list) => {
                let follows_like = matches!(previous, Some(Block::List(earlier)) if earlier.start.is_some() == list.start.is_some());
                alternate = follows_like && !alternate;
                list.write(&mut out, alternate, render);
            }
            other => other.render(&mut out),
        }
        previous = Some(block);
    }
    out
}

impl Block {
    pub(super) fn into_paragraph(self) -> Option<Phrasing> {
        match self {
            Block::Paragraph(phrasing) => Some(phrasing),
            _ => None,
        }
    }

    fn render(&self, out: &mut String) {
        match self {
            Block::Paragraph(phrasing) => {
                out.push_str(&inline::render(phrasing, Setting::Paragraph));
            }
            Block::Heading(level, phrasing) => {
                out.push_str(&"#".repeat(*level));
                out.push(' ');
                out.push_str(&inline::render(phrasing, Setting::Heading));
            }
            Block::Quote(blocks) => prefixed(out, "> ", "> ", &render(blocks)),
            Block::List(list) => list.write(out, false, render),
            Block::Code(code) => {
                let fence = "`".repeat(longest_backtick_run(code).max(2) + 1);
                out.push_str(&fence);
                out.push('\n');
                out.push_str(code);
                out.push('\n');
                out.push_str(&fence);
            }
            Block::Rule => out.push_str("***"),
            Block::Table(rows) => table(out, rows),
        }
    }
}

impl List {
    pub(super) fn write(
        &self,
        out: &mut String,
        alternate: bool,
        content: impl Fn(&[Block]) -> String,
    ) {
        let tight = self
            .items
            .iter()
            .all(|item| matches!(item.as_slice(), [Block::Paragraph(_)]));
        let mut number = self.start;
        for (index, item) in self.items.iter().enumerate() {
            if index > 0 {
                out.push_str(if tight { "\n" } else { "\n\n" });
            }
            let marker = match (&mut number, alternate) {
                (Some(number), _) => {
                    let marker = format!("{number}{} ", if alternate { ')' } else { '.' });
                    *number = (*number + 1).min(LAST_NUMBER);
                    marker
                }
                (None, false) => "- ".to_owned(),
                (None, true) => "* ".to_owned(),
            };
            prefixed(out, &marker, &" ".repeat(marker.len()), &content(item));
        }
    }
}

pub(super) fn prefixed(out: &mut String, first: &str, rest: &str, content: &str) {
    for (index, line) in content.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let prefix = if index == 0 { first } else { rest };
        if line.is_empty() {
            out.push_str(prefix.trim_end());
        } else {
            out.push_str(prefix);
            out.push_str(line);
        }
    }
}

fn table(out: &mut String, rows: &[Vec<Phrasing>]) {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push('|');
        for column in 0..columns {
            out.push(' ');
            if let Some(cell) = row.get(column) {
                out.push_str(&inline::render(cell, Setting::Cell));
            }
            out.push_str(" |");
        }
        if index == 0 {
            out.push_str("\n|");
            out.push_str(&" --- |".repeat(columns));
        }
    }
}
