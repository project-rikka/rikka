use crate::editing::boxes::{GraphemeBox, row_boxes, with_boxes};
use crate::editing::cursor::{Affinity, Caret, Cursor, Movement, SelectionRect};
use crate::layout::{ParagraphLayout, Row};

const ADJACENT_EPSILON: f32 = 1e-3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Start,
    End,
}

impl ParagraphLayout {
    pub fn row_of(&self, cursor: Cursor) -> usize {
        let rows = &self.rows;
        if rows.len() <= 1 {
            return 0;
        }
        let index = cursor.index;
        let found = rows.partition_point(|row| (row.text.end as usize) < index);
        if found >= rows.len() {
            return rows.len() - 1;
        }
        if index < rows[found].text.start as usize && found > 0 {
            return found - 1;
        }
        let row = &rows[found];
        let at_end = row.text.end as usize == index;
        let continues = rows
            .get(found + 1)
            .is_some_and(|next| next.text.start as usize == index);
        match (at_end && continues, cursor.affinity) {
            (true, Affinity::After) => found + 1,
            _ => found,
        }
    }

    pub fn caret(&self, cursor: Cursor) -> Caret {
        let row_index = self.row_of(cursor);
        let Some(row) = self.rows.get(row_index) else {
            return Caret {
                x: 0.0,
                row: 0,
                top: 0.0,
                height: self.params.line_height,
            };
        };
        let x = with_boxes(|boxes| {
            row_boxes(self, row, boxes);
            attached(boxes, row, cursor).map_or_else(
                || empty_row_x(row),
                |(found, edge)| edge_x(&boxes[found], edge),
            )
        });
        Caret {
            x,
            row: row_index,
            top: row.top,
            height: row.height,
        }
    }

    pub fn hit(&self, x: f32, y: f32) -> Cursor {
        let Some(row) = self.row_at_y(y) else {
            return Cursor::new(0, Affinity::After);
        };
        let hit = with_boxes(|boxes| {
            row_boxes(self, row, boxes);
            let last = *boxes.last()?;
            Some(match boxes.iter().find(|grapheme| grapheme.right > x) {
                None => (last, false),
                Some(&grapheme) => (grapheme, x < grapheme.middle()),
            })
        });
        let Some((found, left_half)) = hit else {
            return Cursor::new(row.text.start as usize, Affinity::After);
        };
        if found.is_mark() {
            return Cursor::new(found.start, Affinity::Before);
        }
        match (left_half, found.rtl) {
            (true, false) | (false, true) => Cursor::new(found.start, Affinity::After),
            (false, false) | (true, true) => Cursor::new(found.end, Affinity::Before),
        }
    }

    pub fn selection_rects(&self, range: std::ops::Range<usize>, rects: &mut Vec<SelectionRect>) {
        if range.start < range.end {
            with_boxes(|boxes| self.select(&range, boxes, rects));
        }
    }

    pub fn row_start(&self, cursor: Cursor) -> Cursor {
        self.rows.get(self.row_of(cursor)).map_or(cursor, |row| {
            Cursor::new(row.text.start as usize, Affinity::After)
        })
    }

    pub fn row_end(&self, cursor: Cursor) -> Cursor {
        self.rows.get(self.row_of(cursor)).map_or(cursor, |row| {
            Cursor::new(row.text.end as usize, Affinity::Before)
        })
    }

    fn select(
        &self,
        range: &std::ops::Range<usize>,
        boxes: &mut Vec<GraphemeBox>,
        rects: &mut Vec<SelectionRect>,
    ) {
        for (index, row) in self.rows.iter().enumerate() {
            let text = row.text_range();
            if text.start >= range.end || text.end < range.start {
                continue;
            }
            row_boxes(self, row, boxes);
            let mut open: Option<SelectionRect> = None;
            for grapheme in boxes.iter() {
                let selected = !grapheme.is_mark()
                    && grapheme.start >= range.start
                    && grapheme.end <= range.end;
                match (selected, open.as_mut()) {
                    (true, Some(rect))
                        if (rect.x + rect.width - grapheme.left).abs() < ADJACENT_EPSILON =>
                    {
                        rect.width = grapheme.right - rect.x;
                    }
                    (true, _) => {
                        rects.extend(open.take());
                        open = Some(SelectionRect {
                            row: index,
                            x: grapheme.left,
                            y: row.top,
                            width: grapheme.right - grapheme.left,
                            height: row.height,
                        });
                    }
                    (false, _) => rects.extend(open.take()),
                }
            }
            rects.extend(open);
        }
    }

    pub fn visual_step(&self, cursor: Cursor, movement: Movement) -> Option<Cursor> {
        let row_index = self.row_of(cursor);
        let row = self.rows.get(row_index)?;
        let within = with_boxes(|boxes| {
            row_boxes(self, row, boxes);
            boxes.retain(|grapheme| !grapheme.is_mark());
            let (found, edge) = attached(boxes, row, cursor)?;
            let boundary = match (edge, boxes[found].rtl) {
                (Edge::Start, false) | (Edge::End, true) => found,
                (Edge::End, false) | (Edge::Start, true) => found + 1,
            };
            match movement {
                Movement::Right if boundary < boxes.len() => {
                    Some(right_edge_cursor(&boxes[boundary]))
                }
                Movement::Left if boundary > 0 => Some(left_edge_cursor(&boxes[boundary - 1])),
                _ => None,
            }
        });
        if within.is_some() {
            return within;
        }
        let forward = matches!(movement, Movement::Right) != row.rtl;
        if forward {
            self.rows
                .get(row_index + 1)
                .map(|next| Cursor::new(next.text.start as usize, Affinity::After))
        } else {
            row_index.checked_sub(1).map(|previous| {
                Cursor::new(self.rows[previous].text.end as usize, Affinity::Before)
            })
        }
    }

    fn row_at_y(&self, y: f32) -> Option<&Row> {
        let found = self.rows.partition_point(|row| row.bottom() <= y);
        self.rows.get(found.min(self.rows.len().checked_sub(1)?))
    }
}

fn attached(boxes: &[GraphemeBox], row: &Row, cursor: Cursor) -> Option<(usize, Edge)> {
    let index = cursor
        .index
        .clamp(row.text.start as usize, row.text.end as usize);
    let containing = boxes
        .iter()
        .find(|grapheme| !grapheme.is_mark() && grapheme.start < index && index < grapheme.end);
    let index = containing.map_or(index, |grapheme| grapheme.start);
    let starting = || {
        boxes
            .iter()
            .position(|grapheme| !grapheme.is_mark() && grapheme.start == index)
            .map(|found| (found, Edge::Start))
    };
    let ending = || {
        boxes
            .iter()
            .position(|grapheme| !grapheme.is_mark() && grapheme.end == index)
            .map(|found| (found, Edge::End))
    };
    match (cursor.affinity, containing.is_some()) {
        (_, true) | (Affinity::After, false) => starting().or_else(ending),
        (Affinity::Before, false) => ending().or_else(starting),
    }
}

fn edge_x(grapheme: &GraphemeBox, edge: Edge) -> f32 {
    match (edge, grapheme.rtl) {
        (Edge::Start, false) | (Edge::End, true) => grapheme.left,
        (Edge::End, false) | (Edge::Start, true) => grapheme.right,
    }
}

fn empty_row_x(row: &Row) -> f32 {
    if row.rtl { row.right() } else { row.left }
}

fn right_edge_cursor(grapheme: &GraphemeBox) -> Cursor {
    if grapheme.rtl {
        Cursor::new(grapheme.start, Affinity::After)
    } else {
        Cursor::new(grapheme.end, Affinity::Before)
    }
}

fn left_edge_cursor(grapheme: &GraphemeBox) -> Cursor {
    if grapheme.rtl {
        Cursor::new(grapheme.end, Affinity::Before)
    } else {
        Cursor::new(grapheme.start, Affinity::After)
    }
}
