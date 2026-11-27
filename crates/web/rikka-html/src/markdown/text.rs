use super::block::{Block, prefixed};
use super::phrasing::{Phrasing, Piece};

const CELL_SEPARATOR: &str = " | ";

pub(super) fn render(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        match block {
            Block::Paragraph(phrasing) => out.push_str(&phrased(phrasing, "\n")),
            Block::Heading(_, phrasing) => out.push_str(&phrased(phrasing, " ")),
            Block::Quote(blocks) => prefixed(&mut out, "> ", "> ", &render(blocks)),
            Block::List(list) => list.write(&mut out, false, render),
            Block::Code(code) => out.push_str(code),
            Block::Rule => out.push_str("---"),
            Block::Picture { alt, .. } => out.push_str(alt),
            Block::Table(rows) => {
                let lines: Vec<String> = rows
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|cell| phrased(cell, " "))
                            .collect::<Vec<_>>()
                            .join(CELL_SEPARATOR)
                    })
                    .collect();
                out.push_str(&lines.join("\n"));
            }
        }
    }
    out
}

fn phrased(phrasing: &Phrasing, separator: &str) -> String {
    let mut out = String::new();
    let mut link: Option<(&str, usize)> = None;
    for piece in &phrasing.pieces {
        let (text, target) = match piece {
            Piece::Text { text, style } => (Some(text.as_str()), style.link.as_deref()),
            Piece::Break => (None, None),
        };
        if link.is_some_and(|(url, _)| text.is_none() || target != Some(url)) {
            annotate(&mut out, link.take());
        }
        match text {
            Some(text) => {
                if link.is_none() {
                    link = target.map(|url| (url, out.len()));
                }
                out.push_str(text);
            }
            None => out.push_str(separator),
        }
    }
    annotate(&mut out, link);
    out
}

fn annotate(out: &mut String, link: Option<(&str, usize)>) {
    let Some((url, start)) = link else {
        return;
    };
    let shown = out[start..].trim();
    if shown != url && url.strip_prefix("mailto:") != Some(shown) {
        out.push_str(" <");
        out.push_str(url);
        out.push('>');
    }
}
