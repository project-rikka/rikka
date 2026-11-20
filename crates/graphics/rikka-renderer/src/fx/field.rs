use std::ops::Range;

use rikka_types::{Lens, Point, Rect, RectParts, RefractionProfile, RoundedRect, Size};

const CELL: f32 = 32.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Insets {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

impl Insets {
    fn uniform(by: f32) -> Self {
        Self {
            left: by,
            top: by,
            right: by,
            bottom: by,
        }
    }

    pub(crate) fn expand(self, rect: Rect) -> Rect {
        Rect::new(
            rect.min_x() - self.left,
            rect.min_y() - self.top,
            rect.width() + self.left + self.right,
            rect.height() + self.top + self.bottom,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RefractionField {
    pane: Rect,
    centre: Point,
    half: Size,
    flat_core: Option<Rect>,
    corner: f32,
    rim: f32,
    bevel_displacement: f32,
    bulge_displacement: f32,
    bulge_rate: f32,
    bending: f32,
    directional: bool,
}

impl RefractionField {
    pub(crate) fn directional(
        shape: RoundedRect,
        lens: Lens,
        profile: RefractionProfile,
    ) -> Option<RefractionField> {
        RefractionField::shaped(shape, lens, profile, true)
    }

    pub(crate) fn isotropic(
        shape: RoundedRect,
        lens: Lens,
        profile: RefractionProfile,
    ) -> Option<RefractionField> {
        RefractionField::shaped(shape, lens, profile, false)
    }

    fn shaped(
        shape: RoundedRect,
        lens: Lens,
        profile: RefractionProfile,
        directional: bool,
    ) -> Option<RefractionField> {
        let needed = match directional {
            true => profile.bevel_displacement > profile.bulge_displacement,
            false => profile.bevel_displacement > 0.0,
        };
        if !needed {
            return None;
        }
        let rect = shape.rect;
        let corner = shape.radii.clamp_for_size(rect.size).max() * RoundedRect::CORNER_REACH;
        let flat_core = rect.inflate(-(profile.bevel_width + corner));
        Some(RefractionField {
            pane: rect,
            centre: Point::new(
                rect.min_x() + rect.width() * 0.5,
                rect.min_y() + rect.height() * 0.5,
            ),
            half: Size::new(rect.width() * 0.5, rect.height() * 0.5),
            flat_core: (!flat_core.is_empty()).then_some(flat_core),
            corner,
            rim: profile.bevel_width,
            bevel_displacement: profile.bevel_displacement,
            bulge_displacement: profile.bulge_displacement,
            bulge_rate: lens.bulge() * (1.0 + lens.dispersion()),
            bending: lens.refraction_strength(),
            directional,
        })
    }

    pub(crate) fn outward_reach(&self, drawn: Rect) -> Insets {
        let mut reached = drawn;
        for &piece in self.subtract_flat_core(drawn).as_slice() {
            self.each_cell(piece, |cell, bevel| {
                reached = reached.union(&bevel.expand(cell));
            });
        }
        Insets {
            left: drawn.min_x() - reached.min_x(),
            top: drawn.min_y() - reached.min_y(),
            right: reached.max_x() - drawn.max_x(),
            bottom: reached.max_y() - drawn.max_y(),
        }
    }

    pub(crate) fn displaced_sources(&self, changed: Rect, into: &mut Vec<Rect>) {
        let pieces = self.subtract_flat_core(changed);
        for &piece in pieces.as_slice() {
            let sources = match self.directional {
                true => self.swelled_sources(piece),
                false => self.kept_cell_bounds(
                    piece.inflate(self.bevel_displacement + self.bulge_displacement),
                ),
            };
            into.extend(sources);
        }
    }

    fn swelled_sources(&self, piece: Rect) -> Option<Rect> {
        let mut sources: Option<Rect> = None;
        let search = self.bevel_displacement.max(self.bulge_displacement);
        self.each_cell(piece.inflate(search), |cell, bevel| {
            if self.with_swell(cell, bevel).expand(cell).intersects(&piece) {
                sources = Some(sources.map_or(cell, |all| all.union(&cell)));
            }
        });
        sources
    }

    fn kept_cell_bounds(&self, region: Rect) -> Option<Rect> {
        let region = region.intersection(&self.pane)?;
        let columns = GridSpan::new(region.min_x(), region.max_x(), self.pane.min_x());
        let rows = GridSpan::new(region.min_y(), region.max_y(), self.pane.min_y());
        let (mut across, mut down) = (columns.cells(), rows.cells());
        if let Some(core) = self.flat_core
            && let Some(core_across) = columns.inside(core.min_x(), core.max_x())
            && let Some(core_down) = rows.inside(core.min_y(), core.max_y())
        {
            let (whole_across, whole_down) = (core_across == across, core_down == down);
            if whole_across && whole_down {
                return None;
            }
            if whole_down {
                across = trimmed(across, core_across);
            }
            if whole_across {
                down = trimmed(down, core_down);
            }
        }
        let (left, right) = (
            columns.clipped(across.start).0,
            columns.clipped(across.end - 1).1,
        );
        let (top, bottom) = (rows.clipped(down.start).0, rows.clipped(down.end - 1).1);
        Some(Rect::new(left, top, right - left, bottom - top))
    }

    fn with_swell(&self, cell: Rect, bevel: Insets) -> Insets {
        let swell_left = (cell.max_x() - self.centre.x).max(0.0) * self.bulge_rate;
        let swell_right = (self.centre.x - cell.min_x()).max(0.0) * self.bulge_rate;
        let swell_top = (cell.max_y() - self.centre.y).max(0.0) * self.bulge_rate;
        let swell_bottom = (self.centre.y - cell.min_y()).max(0.0) * self.bulge_rate;
        Insets {
            left: bevel.left.max(swell_left),
            right: bevel.right.max(swell_right),
            top: bevel.top.max(swell_top),
            bottom: bevel.bottom.max(swell_bottom),
        }
    }

    fn each_cell(&self, region: Rect, mut visit: impl FnMut(Rect, Insets)) {
        let Some(region) = region.intersection(&self.pane) else {
            return;
        };
        let col_start = ((region.min_x() - self.pane.min_x()) / CELL).floor() as i32;
        let col_end = ((region.max_x() - self.pane.min_x()) / CELL).ceil() as i32;
        let row_start = ((region.min_y() - self.pane.min_y()) / CELL).floor() as i32;
        let row_end = ((region.max_y() - self.pane.min_y()) / CELL).ceil() as i32;
        let skip_cols = self.flat_core.map(|core| {
            let left = ((core.min_x() - self.pane.min_x()) / CELL).ceil() as i32;
            let right = ((core.max_x() - self.pane.min_x()) / CELL).floor() as i32;
            (left, right)
        });
        for row in row_start..row_end {
            let row_y = self.pane.min_y() + row as f32 * CELL;
            let in_core_y = self.flat_core.is_some_and(|core| {
                let top = row_y.max(region.min_y());
                let bottom = (row_y + CELL).min(region.max_y());
                top >= core.min_y() && bottom <= core.max_y()
            });
            let mut column = col_start;
            while column < col_end {
                if in_core_y
                    && let Some((left, right)) = skip_cols
                    && column >= left
                    && column < right
                {
                    column = right;
                    continue;
                }
                let whole = Rect::new(self.pane.min_x() + column as f32 * CELL, row_y, CELL, CELL);
                column += 1;
                let Some(cell) = whole.intersection(&region) else {
                    continue;
                };
                if self
                    .flat_core
                    .is_some_and(|flat_core| flat_core.contains_rect(&cell))
                {
                    continue;
                }
                if !self.directional {
                    visit(cell, Insets::uniform(self.bevel_displacement));
                    continue;
                }
                let reach = self.displacement_at(cell);
                if reach <= 0.0 {
                    continue;
                }
                let (along, across) = self.axis_weights(cell);
                visit(
                    cell,
                    Insets {
                        left: side(cell.min_x() < self.centre.x, reach * along),
                        right: side(cell.max_x() > self.centre.x, reach * along),
                        top: side(cell.min_y() < self.centre.y, reach * across),
                        bottom: side(cell.max_y() > self.centre.y, reach * across),
                    },
                );
            }
        }
    }

    fn displacement_at(&self, region: Rect) -> f32 {
        let straight = (region.min_x() - self.pane.min_x())
            .min(self.pane.max_x() - region.max_x())
            .min(region.min_y() - self.pane.min_y())
            .min(self.pane.max_y() - region.max_y())
            .max(0.0);
        let deep = (straight - self.corner).max(0.0);
        let u = 1.0 - (deep / self.rim.max(1.0e-6)).clamp(0.0, 1.0);
        if u <= 0.0 {
            return 0.0;
        }
        let tangent = u / (1.0 - u * u).sqrt().max(0.002);
        (tangent * self.bending).min(1.0) * u * u * self.bevel_displacement
    }

    fn axis_weights(&self, region: Rect) -> (f32, f32) {
        let (x_near, x_far) = spans(
            region.min_x() - self.centre.x,
            region.max_x() - self.centre.x,
        );
        let (y_near, y_far) = spans(
            region.min_y() - self.centre.y,
            region.max_y() - self.centre.y,
        );
        let (x_near, x_far) = (x_near / self.half.width, x_far / self.half.width);
        let (y_near, y_far) = (y_near / self.half.height, y_far / self.half.height);
        (
            axis_weight(x_far, y_near, self.half.width, self.half.height),
            axis_weight(y_far, x_near, self.half.height, self.half.width),
        )
    }

    fn subtract_flat_core(&self, region: Rect) -> RectParts {
        match self.flat_core {
            Some(core) => region.subtract_parts(&core),
            None => RectParts::whole(region),
        }
    }
}

#[derive(Clone, Copy)]
struct GridSpan {
    origin: f32,
    low: f32,
    high: f32,
    first: i32,
    end: i32,
}

impl GridSpan {
    fn new(low: f32, high: f32, origin: f32) -> Self {
        let mut span = Self {
            origin,
            low,
            high,
            first: ((low - origin) / CELL).floor() as i32,
            end: ((high - origin) / CELL).ceil() as i32,
        };
        while span.first < span.end && span.is_empty(span.first) {
            span.first += 1;
        }
        while span.end > span.first && span.is_empty(span.end - 1) {
            span.end -= 1;
        }
        span
    }

    fn cells(&self) -> Range<i32> {
        self.first..self.end
    }

    fn clipped(&self, cell: i32) -> (f32, f32) {
        let start = self.origin + cell as f32 * CELL;
        (start.max(self.low), (start + CELL).min(self.high))
    }

    fn is_empty(&self, cell: i32) -> bool {
        let (from, to) = self.clipped(cell);
        from >= to
    }

    fn inside(&self, low: f32, high: f32) -> Option<Range<i32>> {
        let mut inside = self.cells().filter(|&cell| {
            let (from, to) = self.clipped(cell);
            from >= low && to <= high
        });
        let first = inside.next()?;
        let last = inside.next_back().unwrap_or(first);
        Some(first..last + 1)
    }
}

fn trimmed(cells: Range<i32>, core: Range<i32>) -> Range<i32> {
    match (core.start == cells.start, core.end == cells.end) {
        (true, _) => core.end..cells.end,
        (_, true) => cells.start..core.start,
        _ => cells,
    }
}

fn side(faces: bool, reach: f32) -> f32 {
    match faces {
        true => reach,
        false => 0.0,
    }
}

fn spans(from: f32, to: f32) -> (f32, f32) {
    let far = from.abs().max(to.abs());
    match from <= 0.0 && to >= 0.0 {
        true => (0.0, far),
        false => (from.abs().min(to.abs()), far),
    }
}

fn axis_weight(along: f32, across: f32, half_along: f32, half_across: f32) -> f32 {
    let along = along.min(1.0).powi(3) / half_along.max(1.0e-6);
    let across = across.min(1.0).powi(3) / half_across.max(1.0e-6);
    match along > 0.0 {
        true => along / (along * along + across * across).sqrt(),
        false => 0.0,
    }
}
