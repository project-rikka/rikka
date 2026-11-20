use rikka_compositor_geometry::{Logical, Point, Rect, Size};

const PADDING: i32 = 32;
const GAP: i32 = 16;
const LABEL_HEIGHT: i32 = 26;
const INSET: i32 = 8;
const MAX_CARDS: usize = 9;
const MIN_PICTURE_WIDTH: i32 = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub card: Rect<i32, Logical>,
    pub picture: Rect<i32, Logical>,
    pub label: Rect<i32, Logical>,
}

pub fn slots(zone: Rect<i32, Logical>, aspect: f64, count: usize) -> Vec<Slot> {
    let count = count.min(MAX_CARDS);
    if count == 0 || zone.size.w <= 2 * PADDING || zone.size.h <= 2 * PADDING {
        return Vec::new();
    }
    let inner = Rect::new(
        zone.loc + Point::new(PADDING, PADDING),
        Size::new(zone.size.w - 2 * PADDING, zone.size.h - 2 * PADDING),
    );
    let rows = rows_for(inner.size, aspect, count);
    let columns = count.div_ceil(rows);
    let cell = Size::new(
        (inner.size.w - (columns as i32 - 1) * GAP) / columns as i32,
        (inner.size.h - (rows as i32 - 1) * GAP) / rows as i32,
    );
    if cell.w <= INSET * 2 || cell.h <= LABEL_HEIGHT + INSET * 3 {
        return Vec::new();
    }

    (0..count)
        .map(|index| {
            let (column, row) = (index % columns, index / columns);
            let in_row = (count - row * columns).min(columns);
            let span = in_row as i32 * cell.w + (in_row as i32 - 1) * GAP;
            let left = inner.loc.x + (inner.size.w - span) / 2 + column as i32 * (cell.w + GAP);
            let top = inner.loc.y + row as i32 * (cell.h + GAP);
            slot(Rect::new(Point::new(left, top), cell), aspect)
        })
        .collect()
}

fn rows_for(room: Size<i32, Logical>, aspect: f64, count: usize) -> usize {
    let picture_width = |rows: usize| {
        let columns = count.div_ceil(rows);
        let wide = (room.w - (columns as i32 - 1) * GAP) / columns as i32 - 2 * INSET;
        let tall = (room.h - (rows as i32 - 1) * GAP) / rows as i32 - LABEL_HEIGHT - 2 * INSET;
        (wide > 0 && tall > 0).then(|| wide.min((f64::from(tall) * aspect) as i32))
    };
    let mut widest: Option<(i32, usize)> = None;
    for rows in 1..=count {
        let Some(width) = picture_width(rows) else {
            continue;
        };
        if width >= MIN_PICTURE_WIDTH {
            return rows;
        }
        if widest.is_none_or(|(widest_width, _)| width > widest_width) {
            widest = Some((width, rows));
        }
    }
    widest.map_or(count, |(_, rows)| rows)
}

fn slot(cell: Rect<i32, Logical>, aspect: f64) -> Slot {
    let room: Size<i32, Logical> = Size::new(
        cell.size.w - 2 * INSET,
        cell.size.h - LABEL_HEIGHT - 2 * INSET,
    );
    let picture = fitted(Rect::from_size(room), aspect).size.to_i32_round();
    let card = Rect::new(
        Point::new(
            cell.loc.x + (cell.size.w - picture.w) / 2 - INSET,
            cell.loc.y + (cell.size.h - picture.h - LABEL_HEIGHT) / 2 - INSET,
        ),
        Size::new(picture.w + 2 * INSET, picture.h + LABEL_HEIGHT + 2 * INSET),
    );
    Slot {
        card,
        picture: Rect::new(card.loc + Point::new(INSET, INSET), picture),
        label: Rect::new(
            card.loc + Point::new(INSET, INSET + picture.h),
            Size::new(picture.w, LABEL_HEIGHT),
        ),
    }
}

pub fn fitted(room: Rect<i32, Logical>, aspect: f64) -> Rect<f64, Logical> {
    if !aspect.is_finite() || aspect <= 0.0 {
        return room.to_f64();
    }
    let (wide, tall) = (
        f64::from(room.size.h) * aspect,
        f64::from(room.size.w) / aspect,
    );
    let size = if wide <= f64::from(room.size.w) {
        (wide, f64::from(room.size.h))
    } else {
        (f64::from(room.size.w), tall)
    };
    Rect::new(
        Point::new(
            f64::from(room.loc.x) + (f64::from(room.size.w) - size.0) / 2.0,
            f64::from(room.loc.y) + (f64::from(room.size.h) - size.1) / 2.0,
        ),
        Size::new(size.0, size.1),
    )
}

pub fn shape_of(size: Size<i32, Logical>) -> f64 {
    if size.h <= 0 {
        return 0.0;
    }
    f64::from(size.w) / f64::from(size.h)
}
