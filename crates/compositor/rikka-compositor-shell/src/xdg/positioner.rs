use rikka_compositor_geometry::{Logical, Point, Rect, Size};
use rikka_compositor_protocols::xdg_positioner::{Anchor, ConstraintAdjustment, Gravity};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Positioner {
    pub size: Size<i32, Logical>,
    pub anchor_rect: Rect<i32, Logical>,
    pub anchor: Anchor,
    pub gravity: Gravity,
    pub constraint_adjustment: ConstraintAdjustment,
    pub offset: Point<i32, Logical>,
    pub reactive: bool,
    pub parent_size: Option<Size<i32, Logical>>,
    pub parent_configure: Option<u32>,
}

impl Default for Positioner {
    fn default() -> Self {
        Self {
            size: Size::zero(),
            anchor_rect: Rect::zero(),
            anchor: Anchor::None,
            gravity: Gravity::None,
            constraint_adjustment: ConstraintAdjustment::empty(),
            offset: Point::zero(),
            reactive: false,
            parent_size: None,
            parent_configure: None,
        }
    }
}

impl Positioner {
    pub fn placement(&self) -> Rect<i32, Logical> {
        let (anchor_x, anchor_y) = anchor_sides(self.anchor);
        let (gravity_x, gravity_y) = gravity_sides(self.gravity);
        let x = self.horizontal(anchor_x, gravity_x).start();
        let y = self.vertical(anchor_y, gravity_y).start();
        Rect::new(Point::new(x, y), self.size)
    }

    pub fn geometry(&self, constraint: Rect<i32, Logical>) -> Rect<i32, Logical> {
        let adjustment = self.constraint_adjustment;
        let (anchor_x, anchor_y) = anchor_sides(self.anchor);
        let (gravity_x, gravity_y) = gravity_sides(self.gravity);
        let (x, width) = self.horizontal(anchor_x, gravity_x).adjust(
            Bounds::new(constraint.loc.x, constraint.size.w),
            Adjustments {
                flip: adjustment.contains(ConstraintAdjustment::FLIP_X),
                slide: adjustment.contains(ConstraintAdjustment::SLIDE_X),
                resize: adjustment.contains(ConstraintAdjustment::RESIZE_X),
            },
        );
        let (y, height) = self.vertical(anchor_y, gravity_y).adjust(
            Bounds::new(constraint.loc.y, constraint.size.h),
            Adjustments {
                flip: adjustment.contains(ConstraintAdjustment::FLIP_Y),
                slide: adjustment.contains(ConstraintAdjustment::SLIDE_Y),
                resize: adjustment.contains(ConstraintAdjustment::RESIZE_Y),
            },
        );
        Rect::new(Point::new(x, y), Size::new(width, height))
    }

    fn horizontal(&self, anchor: Side, gravity: Side) -> Axis {
        Axis {
            anchor_start: self.anchor_rect.loc.x,
            anchor_length: self.anchor_rect.size.w,
            anchor,
            gravity,
            offset: self.offset.x,
            length: self.size.w,
        }
    }

    fn vertical(&self, anchor: Side, gravity: Side) -> Axis {
        Axis {
            anchor_start: self.anchor_rect.loc.y,
            anchor_length: self.anchor_rect.size.h,
            anchor,
            gravity,
            offset: self.offset.y,
            length: self.size.h,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Start,
    Center,
    End,
}

impl Side {
    fn inverted(self) -> Self {
        match self {
            Self::Start => Self::End,
            Self::Center => Self::Center,
            Self::End => Self::Start,
        }
    }
}

fn anchor_sides(anchor: Anchor) -> (Side, Side) {
    match anchor {
        Anchor::None => (Side::Center, Side::Center),
        Anchor::Top => (Side::Center, Side::Start),
        Anchor::Bottom => (Side::Center, Side::End),
        Anchor::Left => (Side::Start, Side::Center),
        Anchor::Right => (Side::End, Side::Center),
        Anchor::TopLeft => (Side::Start, Side::Start),
        Anchor::BottomLeft => (Side::Start, Side::End),
        Anchor::TopRight => (Side::End, Side::Start),
        Anchor::BottomRight => (Side::End, Side::End),
    }
}

fn gravity_sides(gravity: Gravity) -> (Side, Side) {
    match gravity {
        Gravity::None => (Side::Center, Side::Center),
        Gravity::Top => (Side::Center, Side::Start),
        Gravity::Bottom => (Side::Center, Side::End),
        Gravity::Left => (Side::Start, Side::Center),
        Gravity::Right => (Side::End, Side::Center),
        Gravity::TopLeft => (Side::Start, Side::Start),
        Gravity::BottomLeft => (Side::Start, Side::End),
        Gravity::TopRight => (Side::End, Side::Start),
        Gravity::BottomRight => (Side::End, Side::End),
    }
}

#[derive(Debug, Clone, Copy)]
struct Adjustments {
    flip: bool,
    slide: bool,
    resize: bool,
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    start: i32,
    end: i32,
}

impl Bounds {
    fn new(start: i32, length: i32) -> Self {
        Self {
            start,
            end: start.saturating_add(length),
        }
    }

    fn overflow(self, start: i32, length: i32) -> (i32, i32) {
        (
            self.start.saturating_sub(start),
            start.saturating_add(length).saturating_sub(self.end),
        )
    }

    fn constrains(self, start: i32, length: i32) -> bool {
        let (before, after) = self.overflow(start, length);
        before > 0 || after > 0
    }
}

#[derive(Debug, Clone, Copy)]
struct Axis {
    anchor_start: i32,
    anchor_length: i32,
    anchor: Side,
    gravity: Side,
    offset: i32,
    length: i32,
}

impl Axis {
    fn start(self) -> i32 {
        let anchor = self
            .anchor_start
            .saturating_add(match self.anchor {
                Side::Start => 0,
                Side::Center => self.anchor_length / 2,
                Side::End => self.anchor_length,
            })
            .saturating_add(self.offset);
        match self.gravity {
            Side::Start => anchor.saturating_sub(self.length),
            Side::Center => anchor.saturating_sub(self.length / 2),
            Side::End => anchor,
        }
    }

    fn flipped(self) -> Self {
        Self {
            anchor: self.anchor.inverted(),
            gravity: self.gravity.inverted(),
            ..self
        }
    }

    fn adjust(self, bounds: Bounds, adjustments: Adjustments) -> (i32, i32) {
        let mut axis = self;
        let mut start = axis.start();
        let mut length = axis.length;
        if adjustments.flip && bounds.constrains(start, length) {
            let flipped = axis.flipped();
            let flipped_start = flipped.start();
            if !bounds.constrains(flipped_start, length) {
                axis = flipped;
                start = flipped_start;
            }
        }
        if adjustments.slide && bounds.constrains(start, length) {
            start = slide(start, length, bounds, axis.gravity);
        }
        if adjustments.resize && bounds.constrains(start, length) {
            let clipped_start = start.max(bounds.start);
            let clipped_end = start.saturating_add(length).min(bounds.end);
            if clipped_end > clipped_start {
                start = clipped_start;
                length = clipped_end.saturating_sub(clipped_start);
            }
        }
        (start, length)
    }
}

fn slide(start: i32, length: i32, bounds: Bounds, gravity: Side) -> i32 {
    match gravity {
        Side::Start => {
            let start = slide_back(start, length, bounds);
            slide_forth(start, length, bounds)
        }
        Side::Center | Side::End => {
            let start = slide_forth(start, length, bounds);
            slide_back(start, length, bounds)
        }
    }
}

fn slide_forth(start: i32, length: i32, bounds: Bounds) -> i32 {
    let (before, after) = bounds.overflow(start, length);
    if before <= 0 {
        return start;
    }
    start.saturating_add(before.min(after.saturating_neg().max(0)))
}

fn slide_back(start: i32, length: i32, bounds: Bounds) -> i32 {
    let (before, after) = bounds.overflow(start, length);
    if after <= 0 {
        return start;
    }
    start.saturating_sub(after.min(before.saturating_neg().max(0)))
}
