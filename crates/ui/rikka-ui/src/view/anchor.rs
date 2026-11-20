use std::hash::Hash;

use rikka_types::{Point, Rect, Size};

use crate::view::hash_of;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnchorEdge {
    #[default]
    BelowStart,
    BelowEnd,
    AboveStart,
    AboveEnd,
    BesideTop,
    BesideLeftTop,
    Center,
    Cover,
}

impl AnchorEdge {
    fn mirrored(self) -> Self {
        match self {
            Self::BelowStart => Self::AboveStart,
            Self::BelowEnd => Self::AboveEnd,
            Self::AboveStart => Self::BelowStart,
            Self::AboveEnd => Self::BelowEnd,
            Self::BesideTop => Self::BesideLeftTop,
            Self::BesideLeftTop => Self::BesideTop,
            Self::Center => Self::Center,
            Self::Cover => Self::Cover,
        }
    }

    fn sideways(self) -> bool {
        matches!(self, Self::BesideTop | Self::BesideLeftTop)
    }

    fn drops(self) -> bool {
        matches!(self, Self::BelowStart | Self::BelowEnd)
    }

    fn rises(self) -> bool {
        matches!(self, Self::AboveStart | Self::AboveEnd)
    }
}

pub(crate) const VIEWPORT_MARGIN: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AnchorWidth {
    #[default]
    Content,
    MatchTarget,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    target: u64,
    edge: AnchorEdge,
    offset: Point,
    width: AnchorWidth,
}

impl Anchor {
    pub fn below(target: impl Hash) -> Self {
        Self::new(target, AnchorEdge::BelowStart)
    }

    pub fn above(target: impl Hash) -> Self {
        Self::new(target, AnchorEdge::AboveStart)
    }

    pub fn beside(target: impl Hash) -> Self {
        Self::new(target, AnchorEdge::BesideTop)
    }

    pub fn centered_on(target: impl Hash) -> Self {
        Self::new(target, AnchorEdge::Center)
    }

    pub fn covering(target: impl Hash) -> Self {
        Self::new(target, AnchorEdge::Cover)
    }

    pub fn new(target: impl Hash, edge: AnchorEdge) -> Self {
        Self {
            target: hash_of(target),
            edge,
            offset: Point::ZERO,
            width: AnchorWidth::Content,
        }
    }

    #[must_use]
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset = Point::new(x, y);
        self
    }

    #[must_use]
    pub fn match_target_width(mut self) -> Self {
        self.width = AnchorWidth::MatchTarget;
        self
    }

    pub(crate) fn target(&self) -> u64 {
        self.target
    }

    pub(crate) fn width(&self) -> AnchorWidth {
        self.width
    }

    pub(crate) fn covers(&self) -> bool {
        self.edge == AnchorEdge::Cover
    }

    pub(crate) fn place(&self, target: Rect, size: Size, viewport: Size) -> Placement {
        let origin = corner(self.edge, target, size, self.offset);
        if self.covers() {
            return Placement {
                origin,
                max_height: None,
                rose: false,
            };
        }
        if viewport.is_empty() {
            return Placement {
                origin,
                max_height: None,
                rose: self.edge.rises(),
            };
        }
        if self.edge == AnchorEdge::Center {
            return kept_inside(origin, size, viewport, whole(viewport), false);
        }
        if self.edge.sideways() {
            let mut x = origin.x;
            if !fits(x, size.width, viewport.width) {
                let mirrored = Point::new(-self.offset.x, self.offset.y);
                let flipped = corner(self.edge.mirrored(), target, size, mirrored).x;
                if fits(flipped, size.width, viewport.width) {
                    x = flipped;
                }
            }
            let target = Point::new(x, origin.y);
            return kept_inside(target, size, viewport, whole(viewport), false);
        }

        let landing = dropped(
            target.max_y() + self.offset.y,
            target.y() - self.offset.y,
            size.height,
            viewport.height,
            self.edge.drops(),
        );
        kept_inside(
            Point::new(origin.x, landing.y),
            size,
            viewport,
            landing.room,
            landing.rose,
        )
    }

    pub(crate) fn toward(&self, target: Rect, rose: bool) -> Option<Point> {
        let offset = self.offset;
        let across = target.center().x + offset.x;
        match self.edge {
            AnchorEdge::BelowStart
            | AnchorEdge::BelowEnd
            | AnchorEdge::AboveStart
            | AnchorEdge::AboveEnd => Some(match rose {
                true => Point::new(across, target.y() - offset.y),
                false => Point::new(across, target.max_y() + offset.y),
            }),
            AnchorEdge::BesideTop => {
                Some(Point::new(target.max_x() + offset.x, target.y() + offset.y))
            }
            AnchorEdge::BesideLeftTop => {
                Some(Point::new(target.x() + offset.x, target.y() + offset.y))
            }
            AnchorEdge::Center | AnchorEdge::Cover => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Placement {
    pub(crate) origin: Point,
    pub(crate) max_height: Option<f32>,
    pub(crate) rose: bool,
}

fn kept_inside(target: Point, size: Size, viewport: Size, room: f32, rose: bool) -> Placement {
    Placement {
        origin: Point::new(
            clamp_into(target.x, size.width, viewport.width),
            clamp_into(target.y, size.height.min(room), viewport.height),
        ),
        max_height: Some(room),
        rose,
    }
}

fn whole(viewport: Size) -> f32 {
    (viewport.height - VIEWPORT_MARGIN * 2.0).max(0.0)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct VerticalPlacement {
    y: f32,
    room: f32,
    rose: bool,
}

fn dropped(
    under: f32,
    over: f32,
    height: f32,
    available: f32,
    prefers_under: bool,
) -> VerticalPlacement {
    let below = (available - under - VIEWPORT_MARGIN).max(0.0);
    let above = (over - VIEWPORT_MARGIN).max(0.0);
    let (preferred, other) = if prefers_under {
        (below, above)
    } else {
        (above, below)
    };
    let stays = height <= preferred || preferred >= other;
    let room = if stays { preferred } else { other };
    let height = height.min(room);
    let rose = stays != prefers_under;
    VerticalPlacement {
        y: if rose { over - height } else { under },
        room,
        rose,
    }
}

pub fn drop_panel(target: Rect, gap: f32, size: Size, viewport: Size) -> DropdownPlacement {
    if viewport.is_empty() {
        return DropdownPlacement::hung(
            Rect::new(target.x(), target.max_y() + gap, size.width, size.height),
            Point::new(target.center().x, target.max_y()),
        );
    }
    let landing = dropped(
        target.max_y() + gap,
        target.y() - gap,
        size.height,
        viewport.height,
        true,
    );
    let edge = match landing.rose {
        true => target.y(),
        false => target.max_y(),
    };
    DropdownPlacement::hung(
        Rect::new(
            clamp_into(target.x(), size.width, viewport.width),
            landing.y,
            size.width,
            size.height.min(landing.room),
        ),
        Point::new(target.center().x, edge),
    )
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropdownPlacement {
    rect: Rect,
    pivot: Point,
}

impl DropdownPlacement {
    pub(crate) fn hung(rect: Rect, from: Point) -> Self {
        Self {
            rect,
            pivot: rect.nearest(from),
        }
    }

    #[must_use]
    pub fn rect(&self) -> Rect {
        self.rect
    }

    #[must_use]
    pub fn pivot(&self) -> Point {
        self.pivot
    }
}

pub fn point_panel(at: Point, size: Size, viewport: Size) -> DropdownPlacement {
    let dropped = drop_panel(Rect::new(at.x, at.y, 0.0, 0.0), 0.0, size, viewport);
    if viewport.is_empty() || fits(at.x, size.width, viewport.width) {
        return dropped;
    }
    let leftward = at.x - size.width;
    let x = match fits(leftward, size.width, viewport.width) {
        true => leftward,
        false => clamp_into(at.x, size.width, viewport.width),
    };
    DropdownPlacement::hung(
        Rect::new(
            x,
            dropped.rect.y(),
            dropped.rect.width(),
            dropped.rect.height(),
        ),
        at,
    )
}

pub fn side_panel(target: Rect, offset: Point, size: Size, viewport: Size) -> Rect {
    let placed = Anchor {
        target: 0,
        edge: AnchorEdge::BesideTop,
        offset,
        width: AnchorWidth::Content,
    }
    .place(target, size, viewport);
    let height = placed
        .max_height
        .map_or(size.height, |room| size.height.min(room));
    Rect::new(placed.origin.x, placed.origin.y, size.width, height)
}

fn corner(edge: AnchorEdge, target: Rect, size: Size, offset: Point) -> Point {
    let (x, y) = match edge {
        AnchorEdge::BelowStart => (target.x(), target.max_y()),
        AnchorEdge::BelowEnd => (target.max_x() - size.width, target.max_y()),
        AnchorEdge::AboveStart => (target.x(), target.y() - size.height),
        AnchorEdge::AboveEnd => (target.max_x() - size.width, target.y() - size.height),
        AnchorEdge::BesideTop => (target.max_x(), target.y()),
        AnchorEdge::BesideLeftTop => (target.x() - size.width, target.y()),
        AnchorEdge::Center => (
            target.x() + (target.width() - size.width) / 2.0,
            target.y() + (target.height() - size.height) / 2.0,
        ),
        AnchorEdge::Cover => (target.x(), target.y()),
    };
    Point::new(x + offset.x, y + offset.y)
}

fn fits(start: f32, extent: f32, available: f32) -> bool {
    start >= 0.0 && start + extent <= available
}

pub(crate) fn clamp_into(start: f32, extent: f32, available: f32) -> f32 {
    if fits(start, extent, available) {
        return start;
    }
    let last = (available - extent - VIEWPORT_MARGIN).max(VIEWPORT_MARGIN);
    start.clamp(VIEWPORT_MARGIN, last)
}
