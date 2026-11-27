use crate::document::{Child, Element, ROW_GROUPS};

use super::block::Block;
use super::css;
use super::phrasing::{Phrasing, Style};
use super::walk::{self, Depth, Flow};

const WIDEST_SPAN: usize = 1000;
const CELL_SEPARATOR: &str = " · ";
const COMPLEX: &[&str] = &[
    "blockquote",
    "dir",
    "dl",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hr",
    "listing",
    "menu",
    "ol",
    "plaintext",
    "pre",
    "table",
    "ul",
    "xmp",
];

pub(super) fn table(element: Element<'_>, style: &Style, flow: &mut Flow, depth: Depth) {
    flow.paragraph_break();
    let mut rows = Vec::new();
    let mut headed = false;
    for child in element.children() {
        match child {
            Child::Element(part) if css::is_hidden(part) => {}
            Child::Element(row) if row.name() == "tr" => rows.push(row),
            Child::Element(group) if ROW_GROUPS.contains(&group.name()) => {
                headed |= group.name() == "thead";
                for inner in group.children() {
                    match inner {
                        Child::Element(row) if row.name() == "tr" => {
                            if !css::is_hidden(row) {
                                rows.push(row);
                            }
                        }
                        other => walk::node(other, style, flow, depth.deeper(1)),
                    }
                }
            }
            Child::Element(columns) if matches!(columns.name(), "colgroup" | "col") => {}
            other => walk::node(other, style, flow, depth),
        }
    }
    flow.paragraph_break();
    let depth = depth.deeper(2);
    match is_data(element, &rows, headed) {
        true => data(&rows, style, flow, depth),
        false => layout(&rows, style, flow, depth),
    }
}

fn cells<'a>(row: Element<'a>) -> impl Iterator<Item = Element<'a>> {
    row.children().filter_map(|child| match child {
        Child::Element(cell) if matches!(cell.name(), "td" | "th") && !css::is_hidden(cell) => {
            Some(cell)
        }
        Child::Element(_) | Child::Text(_) => None,
    })
}

fn is_data(table: Element<'_>, rows: &[Element<'_>], headed: bool) -> bool {
    if matches!(table.attribute("role"), Some("presentation" | "none")) || rows.len() < 2 {
        return false;
    }
    let all_cells = || rows.iter().flat_map(|row| cells(*row));
    (headed || all_cells().any(|cell| cell.name() == "th"))
        && rows.iter().any(|row| cells(*row).nth(1).is_some())
        && all_cells().all(|cell| {
            !cell
                .descendants()
                .any(|inner| COMPLEX.contains(&inner.name()))
        })
}

fn data(rows: &[Element<'_>], style: &Style, flow: &mut Flow, depth: Depth) {
    let mut grid: Vec<Vec<Phrasing>> = Vec::with_capacity(rows.len());
    let mut covered: Vec<usize> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let mut line = Vec::new();
        let mut placed = Vec::new();
        let mut pending = cells(*row);
        let mut column = 0;
        loop {
            if let Some(rest) = covered.get_mut(column).filter(|rest| **rest > 0) {
                *rest -= 1;
                line.push(Phrasing::default());
                column += 1;
                continue;
            }
            let Some(cell) = pending.next() else {
                if column < covered.len() {
                    line.push(Phrasing::default());
                    column += 1;
                    continue;
                }
                break;
            };
            line.push(Phrasing::joined(
                walk::contents(cell, style, depth.without_pictures())
                    .into_iter()
                    .filter_map(Block::into_paragraph),
                " ",
            ));
            let across = count(cell, "colspan").map_or(1, |span| span.clamp(1, WIDEST_SPAN));
            line.extend((1..across).map(|_| Phrasing::default()));
            let rest = rows.len() - index;
            let down = count(cell, "rowspan").map_or(1, |span| match span {
                0 => rest,
                span => span.min(rest),
            });
            placed.push((column..column + across, down - 1));
            column += across;
        }
        for (columns, rest) in placed {
            if covered.len() < columns.end {
                covered.resize(columns.end, 0);
            }
            covered[columns].fill(rest);
        }
        grid.push(line);
    }
    let columns = grid.iter().map(Vec::len).max().unwrap_or(0);
    for line in &mut grid {
        line.resize_with(columns, Phrasing::default);
    }
    flow.push(Block::Table(grid));
}

fn count(cell: Element<'_>, attribute: &str) -> Option<usize> {
    cell.attribute(attribute)
        .and_then(|count| count.trim().parse::<usize>().ok())
}

fn layout(rows: &[Element<'_>], style: &Style, flow: &mut Flow, depth: Depth) {
    for row in rows {
        let mut parts: Vec<Vec<Block>> = Vec::new();
        let mut loose = Flow::default();
        for child in row.children() {
            match child {
                Child::Element(cell) if matches!(cell.name(), "td" | "th") => {
                    parts.push(walk::contents(cell, style, depth));
                }
                other => walk::node(other, style, &mut loose, depth),
            }
        }
        parts.push(loose.finish());
        parts.retain(|blocks| !blocks.is_empty());
        let inline = parts.len() >= 2
            && parts.iter().all(|blocks| {
                matches!(blocks.as_slice(), [Block::Paragraph(phrasing)] if !phrasing.has_breaks())
            });
        if inline {
            let joined = Phrasing::joined(
                parts
                    .into_iter()
                    .flatten()
                    .filter_map(Block::into_paragraph),
                CELL_SEPARATOR,
            );
            flow.push(Block::Paragraph(joined));
        } else {
            for block in parts.into_iter().flatten() {
                flow.push(block);
            }
        }
    }
}
