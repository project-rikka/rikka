use rikka_compositor_geometry::{Logical, Point, Rect, Size};

use rikka_compositor_tiling::EDGE_REACH;

const WIDE: i32 = 1920;

const CORNER_LEAST: f64 = 80.0;
const CORNER_MOST: f64 = 240.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Share {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl Share {
    const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn apply_to(self, area: Rect<i32, Logical>) -> Rect<i32, Logical> {
        let along = |start: f64, span: f64, origin: i32, length: i32| {
            let measure = f64::from(length);
            let near = origin + (start * measure).round() as i32;
            let far = origin + ((start + span) * measure).round() as i32;
            (near, far - near)
        };
        let (x, w) = along(self.x, self.w, area.loc.x, area.size.w);
        let (y, h) = along(self.y, self.h, area.loc.y, area.size.h);
        Rect::new(Point::new(x, y), Size::new(w, h))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    shares: &'static [Share],
}

impl Layout {
    pub fn zone(self, area: Rect<i32, Logical>, zone: usize) -> Rect<i32, Logical> {
        self.shares
            .get(zone)
            .map_or_else(Rect::default, |share| share.apply_to(area))
    }

    pub fn zones(self, area: Rect<i32, Logical>) -> impl Iterator<Item = Rect<i32, Logical>> {
        self.shares.iter().map(move |share| share.apply_to(area))
    }
}

const THIRD: f64 = 1.0 / 3.0;
const TWO_THIRDS: f64 = 2.0 / 3.0;

const SIDE_BY_SIDE: Layout = Layout {
    shares: &[
        Share::new(0.0, 0.0, 0.5, 1.0),
        Share::new(0.5, 0.0, 0.5, 1.0),
    ],
};

const LARGE_LEFT: Layout = Layout {
    shares: &[
        Share::new(0.0, 0.0, TWO_THIRDS, 1.0),
        Share::new(TWO_THIRDS, 0.0, THIRD, 1.0),
    ],
};

const HALF_AND_STACK: Layout = Layout {
    shares: &[
        Share::new(0.0, 0.0, 0.5, 1.0),
        Share::new(0.5, 0.0, 0.5, 0.5),
        Share::new(0.5, 0.5, 0.5, 0.5),
    ],
};

const QUARTERS: Layout = Layout {
    shares: &[
        Share::new(0.0, 0.0, 0.5, 0.5),
        Share::new(0.5, 0.0, 0.5, 0.5),
        Share::new(0.0, 0.5, 0.5, 0.5),
        Share::new(0.5, 0.5, 0.5, 0.5),
    ],
};

const THREE_COLUMNS: Layout = Layout {
    shares: &[
        Share::new(0.0, 0.0, THIRD, 1.0),
        Share::new(THIRD, 0.0, THIRD, 1.0),
        Share::new(TWO_THIRDS, 0.0, THIRD, 1.0),
    ],
};

const WIDE_MIDDLE: Layout = Layout {
    shares: &[
        Share::new(0.0, 0.0, 0.25, 1.0),
        Share::new(0.25, 0.0, 0.5, 1.0),
        Share::new(0.75, 0.0, 0.25, 1.0),
    ],
};

const ONE_ABOVE_ONE: Layout = Layout {
    shares: &[
        Share::new(0.0, 0.0, 1.0, 0.5),
        Share::new(0.0, 0.5, 1.0, 0.5),
    ],
};

const LARGE_TOP: Layout = Layout {
    shares: &[
        Share::new(0.0, 0.0, 1.0, TWO_THIRDS),
        Share::new(0.0, TWO_THIRDS, 1.0, THIRD),
    ],
};

const HALF_AND_ROW: Layout = Layout {
    shares: &[
        Share::new(0.0, 0.0, 1.0, 0.5),
        Share::new(0.0, 0.5, 0.5, 0.5),
        Share::new(0.5, 0.5, 0.5, 0.5),
    ],
};

const NARROW: [Layout; 4] = [SIDE_BY_SIDE, LARGE_LEFT, HALF_AND_STACK, QUARTERS];

const WIDE_SCREEN: [Layout; 6] = [
    SIDE_BY_SIDE,
    LARGE_LEFT,
    HALF_AND_STACK,
    QUARTERS,
    THREE_COLUMNS,
    WIDE_MIDDLE,
];

const UPRIGHT: [Layout; 4] = [ONE_ABOVE_ONE, LARGE_TOP, HALF_AND_ROW, QUARTERS];

pub fn templates(area: Size<i32, Logical>) -> &'static [Layout] {
    if area.h > area.w {
        &UPRIGHT
    } else if area.w >= WIDE {
        &WIDE_SCREEN
    } else {
        &NARROW
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Edged {
    Maximize,
    Zone { layout: Layout, zone: usize },
}

pub fn at_edge(
    screen: Rect<i32, Logical>,
    area: Rect<i32, Logical>,
    at: Point<f64, Logical>,
) -> Option<Edged> {
    if area.size.w <= 0 || area.size.h <= 0 {
        return None;
    }
    let display = screen.to_f64();
    if at.x < display.loc.x - EDGE_REACH
        || at.x > display.loc.x + display.size.w + EDGE_REACH
        || at.y < display.loc.y - EDGE_REACH
        || at.y > display.loc.y + display.size.h + EDGE_REACH
    {
        return None;
    }
    let left = f64::from(area.loc.x);
    let right = f64::from(area.loc.x + area.size.w);
    let top = f64::from(area.loc.y);
    let side = if at.x <= left + EDGE_REACH {
        Some(0)
    } else if at.x >= right - EDGE_REACH {
        Some(1)
    } else {
        None
    };
    let Some(side) = side else {
        return (at.y <= top + EDGE_REACH).then_some(Edged::Maximize);
    };
    let band = (f64::from(area.size.h) / 4.0).clamp(CORNER_LEAST, CORNER_MOST);
    let upper = top + band;
    let lower = f64::from(area.loc.y + area.size.h) - band;
    let (layout, zone) = if at.y <= upper {
        (QUARTERS, side)
    } else if at.y >= lower {
        (QUARTERS, side + 2)
    } else {
        (SIDE_BY_SIDE, side)
    };
    Some(Edged::Zone { layout, zone })
}
