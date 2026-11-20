use rikka_types::{Point, Rect, Size};

use crate::style::{Align, Inset, Justify, Length, Space, Style};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Extent {
    pub width: Length,
    pub height: Length,
}

impl Extent {
    pub const HUG: Self = Self {
        width: Length::HUG,
        height: Length::HUG,
    };

    pub fn new(width: Length, height: Length) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Axis {
    #[default]
    Vertical,
    Horizontal,
}

impl Axis {
    pub fn room(self, bounds: Rect) -> f32 {
        self.length(bounds.size)
    }

    pub fn length(self, size: Size) -> f32 {
        match self {
            Self::Vertical => size.height,
            Self::Horizontal => size.width,
        }
    }

    pub fn coord(self, point: Point) -> f32 {
        match self {
            Self::Vertical => point.y,
            Self::Horizontal => point.x,
        }
    }

    pub fn across(self) -> Self {
        match self {
            Self::Vertical => Self::Horizontal,
            Self::Horizontal => Self::Vertical,
        }
    }

    pub fn point(self, along: f32, across: f32) -> Point {
        match self {
            Self::Vertical => Point::new(across, along),
            Self::Horizontal => Point::new(along, across),
        }
    }
    pub fn rect(self, origin: Point, along: f32, extent: f32, across: f32, breadth: f32) -> Rect {
        let start = origin + self.point(along, across);
        match self {
            Self::Vertical => Rect::new(start.x, start.y, breadth, extent),
            Self::Horizontal => Rect::new(start.x, start.y, extent, breadth),
        }
    }

    pub fn extent(self, extent: Extent) -> Length {
        match self {
            Self::Vertical => extent.height,
            Self::Horizontal => extent.width,
        }
    }

    pub fn inset(self, inset: Inset) -> f32 {
        match self {
            Self::Vertical => inset.vertical(),
            Self::Horizontal => inset.horizontal(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Track {
    Even(u16),
    Fitting(f32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tracks {
    pub along: Axis,
    pub track: Track,
    pub band: Option<f32>,
}

impl Tracks {
    pub fn new(track: Track) -> Self {
        Self {
            along: Axis::Vertical,
            track,
            band: None,
        }
    }

    pub fn lines(lines: usize) -> u16 {
        u16::try_from(lines.max(1)).unwrap_or(u16::MAX)
    }

    #[must_use]
    pub fn along(mut self, along: Axis) -> Self {
        self.along = along;
        self
    }

    #[must_use]
    pub fn band(mut self, band: Option<f32>) -> Self {
        self.band = band;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Flow {
    #[default]
    Row,
    Column,
    Grid(Tracks),
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layout {
    pub flow: Flow,
    pub wrap: bool,
    pub scrolls: Option<Axis>,
    pub gap: Size,
    pub padding: Inset,
    pub margin: Inset,
    pub size: Extent,
    pub min: Extent,
    pub max: Extent,
    pub basis: Length,
    pub grow: f32,
    pub shrink: f32,
    pub align_items: Option<Align>,
    pub align_self: Option<Align>,
    pub justify: Option<Justify>,
    pub inset: Option<Point>,
}

impl Layout {
    pub const DEFAULT: Self = Self {
        flow: Flow::Row,
        wrap: false,
        scrolls: None,
        gap: Size::ZERO,
        padding: Inset::NONE,
        margin: Inset::NONE,
        size: Extent::HUG,
        min: Extent::HUG,
        max: Extent::HUG,
        basis: Length::HUG,
        grow: 0.0,
        shrink: 1.0,
        align_items: None,
        align_self: None,
        justify: None,
        inset: None,
    };

    pub fn row() -> Self {
        Self::DEFAULT
    }

    pub fn column() -> Self {
        Self {
            flow: Flow::Column,
            ..Self::DEFAULT
        }
    }
    pub fn grid(tracks: Tracks) -> Self {
        Self {
            flow: Flow::Grid(tracks),
            ..Self::DEFAULT
        }
    }

    #[must_use]
    pub fn scrolling(mut self, along: Axis) -> Self {
        self.scrolls = Some(along);
        self
    }

    #[must_use]
    pub fn gap(self, space: impl Into<Space>) -> Self {
        let space = space.into();
        self.gap_xy(space, space)
    }

    #[must_use]
    pub fn gap_xy(mut self, x: impl Into<Space>, y: impl Into<Space>) -> Self {
        self.gap = Size::new(x.into().px(), y.into().px());
        self
    }

    #[must_use]
    pub fn gap_exact(mut self, x: f32, y: f32) -> Self {
        self.gap = Size::new(x.max(0.0), y.max(0.0));
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: impl Into<Inset>) -> Self {
        self.padding = padding.into();
        self
    }

    #[must_use]
    pub fn size(mut self, width: impl Into<Length>, height: impl Into<Length>) -> Self {
        self.size = Extent::new(width.into(), height.into());
        self
    }

    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.size.width = width.into();
        self
    }

    #[must_use]
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.size.height = height.into();
        self
    }

    #[must_use]
    pub fn min_width(mut self, width: impl Into<Length>) -> Self {
        self.min.width = width.into();
        self
    }

    #[must_use]
    pub fn min_height(mut self, height: impl Into<Length>) -> Self {
        self.min.height = height.into();
        self
    }

    #[must_use]
    pub fn max_width(mut self, width: impl Into<Length>) -> Self {
        self.max.width = width.into();
        self
    }

    #[must_use]
    pub fn max_height(mut self, height: impl Into<Length>) -> Self {
        self.max.height = height.into();
        self
    }

    #[must_use]
    pub fn grow(mut self, factor: f32) -> Self {
        self.grow = factor.max(0.0);
        self
    }

    #[must_use]
    pub fn shrink(mut self, factor: f32) -> Self {
        self.shrink = factor.max(0.0);
        self
    }

    #[must_use]
    pub fn fixed(self) -> Self {
        self.shrink(0.0)
    }

    #[must_use]
    pub fn align_items(mut self, align: Align) -> Self {
        self.align_items = Some(align);
        self
    }

    #[must_use]
    pub fn justify(mut self, justify: Justify) -> Self {
        self.justify = Some(justify);
        self
    }

    #[must_use]
    pub fn placed_at(mut self, origin: Point) -> Self {
        self.inset = Some(origin);
        self
    }

    #[must_use]
    pub fn with(mut self, props: &Style) -> Self {
        props.apply(&mut self);
        self
    }

    pub(crate) fn anchored(mut self) -> Self {
        if self.inset.is_none() {
            self.inset = Some(Point::ZERO);
        }
        self
    }
}
