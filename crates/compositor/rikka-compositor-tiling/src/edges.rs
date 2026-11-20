use rikka_compositor_geometry::{Logical, Point, Rect};
use rikka_types::ResizeEdge;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Edges: u32 {
        const TOP = 0b0001;
        const BOTTOM = 0b0010;
        const LEFT = 0b0100;
        const RIGHT = 0b1000;
        const TOP_LEFT = Self::TOP.bits() | Self::LEFT.bits();
        const TOP_RIGHT = Self::TOP.bits() | Self::RIGHT.bits();
        const BOTTOM_LEFT = Self::BOTTOM.bits() | Self::LEFT.bits();
        const BOTTOM_RIGHT = Self::BOTTOM.bits() | Self::RIGHT.bits();
    }
}

pub const EDGE_REACH: f64 = 8.0;

impl Edges {
    pub fn near(rect: Rect<i32, Logical>, at: Point<f64, Logical>, reach: f64) -> Self {
        let mut edges = Self::empty();
        let (left, top) = (f64::from(rect.loc.x), f64::from(rect.loc.y));
        let right = left + f64::from(rect.size.w);
        let bottom = top + f64::from(rect.size.h);
        if at.x < left - reach
            || at.x > right + reach
            || at.y < top - reach
            || at.y > bottom + reach
        {
            return edges;
        }
        if (at.x - left).abs() <= reach {
            edges |= Self::LEFT;
        } else if (at.x - right).abs() <= reach {
            edges |= Self::RIGHT;
        }
        if (at.y - top).abs() <= reach {
            edges |= Self::TOP;
        } else if (at.y - bottom).abs() <= reach {
            edges |= Self::BOTTOM;
        }
        edges
    }

    pub fn quadrant(rect: Rect<i32, Logical>, at: Point<f64, Logical>) -> Self {
        let middle = rect.loc.to_f64()
            + Point::new(f64::from(rect.size.w) / 2.0, f64::from(rect.size.h) / 2.0);
        let horizontal = if at.x < middle.x {
            Self::LEFT
        } else {
            Self::RIGHT
        };
        let vertical = if at.y < middle.y {
            Self::TOP
        } else {
            Self::BOTTOM
        };
        horizontal | vertical
    }
}

impl From<ResizeEdge> for Edges {
    fn from(edge: ResizeEdge) -> Self {
        match edge {
            ResizeEdge::Top => Self::TOP,
            ResizeEdge::Bottom => Self::BOTTOM,
            ResizeEdge::Left => Self::LEFT,
            ResizeEdge::Right => Self::RIGHT,
            ResizeEdge::TopLeft => Self::TOP_LEFT,
            ResizeEdge::TopRight => Self::TOP_RIGHT,
            ResizeEdge::BottomLeft => Self::BOTTOM_LEFT,
            ResizeEdge::BottomRight => Self::BOTTOM_RIGHT,
        }
    }
}
