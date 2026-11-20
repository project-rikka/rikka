use crate::region::{Region, Span, merge_row};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: (u32, u32),
    pub to: (u32, u32),
}

impl Region {
    #[must_use]
    pub fn outline(&self) -> Vec<Edge> {
        let footprint = self.footprint();
        let lines = footprint.lines();
        let mut edges = Vec::new();
        let mut open: Vec<(u32, u32)> = Vec::new();
        let mut row_edges = Vec::new();
        let differ = |one: u8, other: u8| match (one > 0) == (other > 0) {
            true => 0,
            false => Span::FULL,
        };
        for y in lines.start..=lines.end {
            let above = match y > lines.start {
                true => footprint.row(y - 1),
                false => &[],
            };
            let here = footprint.row(y);
            merge_row(above, here, differ, |start, end, _| {
                edges.push(Edge {
                    from: (start, y),
                    to: (end, y),
                });
            });
            row_edges.clear();
            row_edges.extend(here.iter().flat_map(|span| [span.start, span.end]));
            open = advance(&open, &row_edges, y, &mut edges);
        }
        edges
    }
}

fn advance(open: &[(u32, u32)], here: &[u32], y: u32, edges: &mut Vec<Edge>) -> Vec<(u32, u32)> {
    let mut next = Vec::with_capacity(here.len());
    let (mut old, mut new) = (0, 0);
    loop {
        let closing = match (open.get(old), here.get(new)) {
            (None, None) => break,
            (Some(&(x, since)), Some(&at)) if x == at => {
                next.push((x, since));
                old += 1;
                new += 1;
                None
            }
            (Some(&pending), None) => Some(pending),
            (Some(&pending), Some(&at)) if pending.0 < at => Some(pending),
            (_, Some(&at)) => {
                next.push((at, y));
                new += 1;
                None
            }
        };
        if let Some((x, since)) = closing {
            edges.push(Edge {
                from: (x, since),
                to: (x, y),
            });
            old += 1;
        }
    }
    next
}
