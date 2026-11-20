use crate::{Point, Rect, Size};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Transform {
    #[default]
    Normal,
    Rotate90,
    Rotate180,
    Rotate270,
    Flipped,
    Flipped90,
    Flipped180,
    Flipped270,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AxisMap {
    pub swapped: bool,
    pub mirrored_x: bool,
    pub mirrored_y: bool,
}

impl Transform {
    pub const ALL: [Self; 8] = [
        Self::Normal,
        Self::Rotate90,
        Self::Rotate180,
        Self::Rotate270,
        Self::Flipped,
        Self::Flipped90,
        Self::Flipped180,
        Self::Flipped270,
    ];

    #[must_use]
    pub const fn from_wayland(value: u32) -> Option<Self> {
        match value {
            0..8 => Some(Self::ALL[value as usize]),
            _ => None,
        }
    }

    #[must_use]
    pub const fn to_wayland(self) -> u32 {
        self.quarter_turns() + if self.is_flipped() { 4 } else { 0 }
    }

    const fn from_parts(flipped: bool, quarter_turns: u32) -> Self {
        Self::ALL[(quarter_turns % 4) as usize + if flipped { 4 } else { 0 }]
    }

    #[must_use]
    pub const fn is_flipped(self) -> bool {
        matches!(
            self,
            Self::Flipped | Self::Flipped90 | Self::Flipped180 | Self::Flipped270
        )
    }

    #[must_use]
    pub const fn quarter_turns(self) -> u32 {
        match self {
            Self::Normal | Self::Flipped => 0,
            Self::Rotate90 | Self::Flipped90 => 1,
            Self::Rotate180 | Self::Flipped180 => 2,
            Self::Rotate270 | Self::Flipped270 => 3,
        }
    }

    #[must_use]
    pub const fn swaps_axes(self) -> bool {
        self.quarter_turns() % 2 == 1
    }

    #[must_use]
    pub const fn inverse(self) -> Self {
        match self {
            Self::Rotate90 => Self::Rotate270,
            Self::Rotate270 => Self::Rotate90,
            other => other,
        }
    }

    #[must_use]
    pub const fn then(self, next: Self) -> Self {
        let turns = match next.is_flipped() {
            true => next.quarter_turns() + 4 - self.quarter_turns(),
            false => next.quarter_turns() + self.quarter_turns(),
        };
        Self::from_parts(self.is_flipped() != next.is_flipped(), turns)
    }

    #[must_use]
    pub const fn axis_map(self) -> AxisMap {
        let (swapped, mirrored_x, mirrored_y) = match self {
            Self::Normal => (false, false, false),
            Self::Rotate90 => (true, false, true),
            Self::Rotate180 => (false, true, true),
            Self::Rotate270 => (true, true, false),
            Self::Flipped => (false, true, false),
            Self::Flipped90 => (true, false, false),
            Self::Flipped180 => (false, false, true),
            Self::Flipped270 => (true, true, true),
        };
        AxisMap {
            swapped,
            mirrored_x,
            mirrored_y,
        }
    }

    #[must_use]
    pub fn map_size(self, size: Size) -> Size {
        match self.swaps_axes() {
            true => Size::new(size.height, size.width),
            false => size,
        }
    }

    #[must_use]
    pub fn map_point(self, point: Point, area: Size) -> Point {
        let axes = self.axis_map();
        let mapped = self.map_size(area);
        let (x, y) = match axes.swapped {
            true => (point.y, point.x),
            false => (point.x, point.y),
        };
        Point::new(
            if axes.mirrored_x { mapped.width - x } else { x },
            if axes.mirrored_y {
                mapped.height - y
            } else {
                y
            },
        )
    }

    #[must_use]
    pub fn map_rect(self, rect: Rect, area: Size) -> Rect {
        let near = self.map_point(Point::new(rect.min_x(), rect.min_y()), area);
        let far = self.map_point(Point::new(rect.max_x(), rect.max_y()), area);
        let (origin, corner) = (near.min(far), near.max(far));
        Rect::new(origin.x, origin.y, corner.x - origin.x, corner.y - origin.y)
    }
}
