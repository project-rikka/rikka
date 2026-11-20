use rikka_types::{Point, Rect, Size};

const PADDING: f32 = 12.0;
pub(crate) const GAP: f32 = 6.0;
const INSET: f32 = 8.0;
const HEADER: f32 = 24.0;
const HEADER_GAP: f32 = 8.0;

const TALLEST: f32 = 150.0;
const SHORTEST: f32 = 64.0;
const SHRINK: f32 = 0.9;
const NARROWEST: f32 = 0.75;
const WIDEST: f32 = 2.0;
const UNKNOWN_SHAPE: f32 = 16.0 / 10.0;
const ACROSS: f32 = 0.9;
const DOWN: f32 = 0.8;

#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub frame: Rect,
    pub header: Rect,
    pub window: Rect,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Arrangement {
    pub panel: Rect,
    pub viewport: Rect,
    pub cards: Vec<Card>,
    rows: Vec<Row>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Row {
    first: usize,
    count: usize,
    top: f32,
    bottom: f32,
}

#[derive(Debug)]
struct Line {
    first: usize,
    widths: Vec<f32>,
    used: f32,
}

#[must_use]
pub fn arrange(room: Size, windows: &[Size]) -> Arrangement {
    let across = room.width * ACROSS - 2.0 * PADDING;
    let down = room.height * DOWN - 2.0 * PADDING;
    if windows.is_empty() || across <= 0.0 || down <= 0.0 {
        return Arrangement::default();
    }
    let mut height = TALLEST.min(down - card_height(0.0)).max(SHORTEST);
    let mut rows = flow(windows, height, across);
    while content_height(&rows, height) > down && height > SHORTEST {
        height = (height * SHRINK).max(SHORTEST);
        rows = flow(windows, height, across);
    }
    place(room, windows, &rows, height, down)
}

impl Arrangement {
    #[must_use]
    pub fn centres(&self) -> Vec<Vec<f32>> {
        self.rows
            .iter()
            .map(|row| {
                self.cards[row.first..row.first + row.count]
                    .iter()
                    .map(|card| card.frame.center().x)
                    .collect()
            })
            .collect()
    }

    #[must_use]
    pub fn reach(&self) -> f32 {
        let bottom = self.rows.last().map_or(0.0, |row| row.bottom);
        (bottom - self.viewport.max_y()).max(0.0)
    }

    #[must_use]
    pub fn scrolled_to(&self, selected: usize, scroll: f32) -> f32 {
        let Some(row) = self
            .rows
            .iter()
            .find(|row| (row.first..row.first + row.count).contains(&selected))
        else {
            return scroll.clamp(0.0, self.reach());
        };
        let (top, bottom) = (
            row.top - self.viewport.min_y(),
            row.bottom - self.viewport.min_y(),
        );
        let visible_height = self.viewport.height();
        let scroll = if top < scroll {
            top
        } else if bottom > scroll + visible_height {
            bottom - visible_height
        } else {
            scroll
        };
        scroll.clamp(0.0, self.reach())
    }

    #[must_use]
    pub fn at(&self, point: Point, scroll: f32) -> Option<usize> {
        if !self.viewport.contains(point) {
            return None;
        }
        let point = Point::new(point.x, point.y + scroll);
        self.cards
            .iter()
            .position(|card| card.frame.contains(point))
    }
}

fn slot_width(window: Size, height: f32, across: f32) -> f32 {
    let shape = match window.width > 0.0 && window.height > 0.0 {
        true => window.width / window.height,
        false => UNKNOWN_SHAPE,
    };
    (shape * height)
        .clamp(NARROWEST * height, WIDEST * height)
        .min(across - 2.0 * INSET)
}

fn card_width(window: Size, height: f32, across: f32) -> f32 {
    slot_width(window, height, across) + 2.0 * INSET
}

fn card_height(height: f32) -> f32 {
    2.0 * INSET + HEADER + HEADER_GAP + height
}

fn flow(windows: &[Size], height: f32, across: f32) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    for (index, window) in windows.iter().enumerate() {
        let width = card_width(*window, height, across);
        match lines.last_mut() {
            Some(line) if line.used + GAP + width <= across => {
                line.widths.push(width);
                line.used += GAP + width;
            }
            _ => lines.push(Line {
                first: index,
                widths: vec![width],
                used: width,
            }),
        }
    }
    lines
}

fn content_height(lines: &[Line], height: f32) -> f32 {
    let count = lines.len() as f32;
    count * card_height(height) + (count - 1.0).max(0.0) * GAP
}

fn place(room: Size, windows: &[Size], lines: &[Line], height: f32, down: f32) -> Arrangement {
    let inner = Size::new(
        lines.iter().map(|line| line.used).fold(0.0, f32::max),
        content_height(lines, height).min(down),
    );
    let panel = Rect::from_center(
        Point::new(room.width / 2.0, room.height / 2.0),
        Size::new(inner.width + 2.0 * PADDING, inner.height + 2.0 * PADDING),
    );
    let panel = Rect::new(
        panel.min_x().round(),
        panel.min_y().round(),
        panel.width(),
        panel.height(),
    );
    let viewport = panel.inflate(-PADDING);
    let mut cards = Vec::with_capacity(windows.len());
    let mut rows = Vec::with_capacity(lines.len());
    let mut top = viewport.min_y();
    for line in lines {
        let mut left = viewport.min_x() + ((viewport.width() - line.used) / 2.0).round();
        for (window, width) in windows[line.first..].iter().zip(&line.widths) {
            cards.push(card(
                Rect::new(left, top, *width, card_height(height)),
                *window,
            ));
            left += width + GAP;
        }
        let bottom = top + card_height(height);
        rows.push(Row {
            first: line.first,
            count: line.widths.len(),
            top,
            bottom,
        });
        top = bottom + GAP;
    }
    Arrangement {
        panel,
        viewport,
        cards,
        rows,
    }
}

fn card(frame: Rect, window: Size) -> Card {
    let header = Rect::new(
        frame.min_x() + INSET,
        frame.min_y() + INSET,
        frame.width() - 2.0 * INSET,
        HEADER,
    );
    let slot = Rect::new(
        header.min_x(),
        header.max_y() + HEADER_GAP,
        header.width(),
        frame.max_y() - INSET - header.max_y() - HEADER_GAP,
    );
    Card {
        frame,
        header,
        window: fitted(window, slot),
    }
}

fn fitted(window: Size, slot: Rect) -> Rect {
    if window.width <= 0.0 || window.height <= 0.0 {
        return slot;
    }
    let scale = (slot.width() / window.width)
        .min(slot.height() / window.height)
        .min(1.0);
    Rect::from_center(
        slot.center(),
        Size::new(window.width * scale, window.height * scale),
    )
}
