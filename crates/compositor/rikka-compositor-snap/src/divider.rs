use rikka_compositor_geometry::{Logical, Point, Rect, Size};

use rikka_compositor_tiling::Edges;

const TOUCH: i32 = 6;
const OVERLAP: i32 = 48;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Along {
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Divider {
    pub along: Along,
    pub at: i32,
    pub from: i32,
    pub to: i32,
}

impl Divider {
    fn spans(self, rect: Rect<i32, Logical>) -> bool {
        let (near, far) = match self.along {
            Along::Vertical => (rect.loc.y, rect.loc.y + rect.size.h),
            Along::Horizontal => (rect.loc.x, rect.loc.x + rect.size.w),
        };
        near.max(self.from) + OVERLAP <= far.min(self.to)
    }

    fn leading(self, rect: Rect<i32, Logical>) -> bool {
        let far = match self.along {
            Along::Vertical => rect.loc.x + rect.size.w,
            Along::Horizontal => rect.loc.y + rect.size.h,
        };
        (far - self.at).abs() <= TOUCH && self.spans(rect)
    }

    fn trailing(self, rect: Rect<i32, Logical>) -> bool {
        let near = match self.along {
            Along::Vertical => rect.loc.x,
            Along::Horizontal => rect.loc.y,
        };
        (near - self.at).abs() <= TOUCH && self.spans(rect)
    }

    pub fn takes(self, delta: Point<f64, Logical>) -> i32 {
        match self.along {
            Along::Vertical => delta.x,
            Along::Horizontal => delta.y,
        }
        .round() as i32
    }
}

pub fn between(
    rects: &[Rect<i32, Logical>],
    at: Point<f64, Logical>,
    reach: f64,
) -> Option<Divider> {
    candidates(rects)
        .into_iter()
        .filter(|divider| {
            let (across, along) = match divider.along {
                Along::Vertical => (at.x, at.y),
                Along::Horizontal => (at.y, at.x),
            };
            (across - f64::from(divider.at)).abs() <= reach
                && along >= f64::from(divider.from)
                && along <= f64::from(divider.to)
        })
        .min_by(|one, other| {
            let distance = |divider: &Divider| match divider.along {
                Along::Vertical => (at.x - f64::from(divider.at)).abs(),
                Along::Horizontal => (at.y - f64::from(divider.at)).abs(),
            };
            distance(one).total_cmp(&distance(other))
        })
}

pub fn pulled(rects: &[Rect<i32, Logical>], index: usize, edges: Edges) -> Option<Divider> {
    let rect = *rects.get(index)?;
    let divider_at = |along: Along, at: i32| {
        candidates(rects)
            .into_iter()
            .find(|divider| divider.along == along && (divider.at - at).abs() <= TOUCH)
    };
    if edges.contains(Edges::LEFT) {
        return divider_at(Along::Vertical, rect.loc.x);
    }
    if edges.contains(Edges::RIGHT) {
        return divider_at(Along::Vertical, rect.loc.x + rect.size.w);
    }
    if edges.contains(Edges::TOP) {
        return divider_at(Along::Horizontal, rect.loc.y);
    }
    if edges.contains(Edges::BOTTOM) {
        return divider_at(Along::Horizontal, rect.loc.y + rect.size.h);
    }
    None
}

pub fn shift(
    rects: &[Rect<i32, Logical>],
    minima: &[Size<i32, Logical>],
    divider: Divider,
    delta: i32,
) -> Vec<Option<Rect<i32, Logical>>> {
    let span = |rect: Rect<i32, Logical>| match divider.along {
        Along::Vertical => rect.size.w,
        Along::Horizontal => rect.size.h,
    };
    let least = |index: usize| {
        let size = minima.get(index).copied().unwrap_or_default();
        match divider.along {
            Along::Vertical => size.w,
            Along::Horizontal => size.h,
        }
        .max(1)
    };

    let mut room = (i32::MIN, i32::MAX);
    for (index, rect) in rects.iter().enumerate() {
        if divider.leading(*rect) {
            room.0 = room.0.max(least(index) - span(*rect));
        }
        if divider.trailing(*rect) {
            room.1 = room.1.min(span(*rect) - least(index));
        }
    }
    let delta = delta.clamp(room.0.min(0), room.1.max(0));

    rects
        .iter()
        .map(|rect| {
            let mut moved = *rect;
            if divider.leading(*rect) {
                match divider.along {
                    Along::Vertical => moved.size.w += delta,
                    Along::Horizontal => moved.size.h += delta,
                }
            } else if divider.trailing(*rect) {
                match divider.along {
                    Along::Vertical => {
                        moved.loc.x += delta;
                        moved.size.w -= delta;
                    }
                    Along::Horizontal => {
                        moved.loc.y += delta;
                        moved.size.h -= delta;
                    }
                }
            }
            (moved != *rect).then_some(moved)
        })
        .collect()
}

fn candidates(rects: &[Rect<i32, Logical>]) -> Vec<Divider> {
    let mut dividers: Vec<Divider> = Vec::new();
    for (one, other) in pairs(rects) {
        for along in [Along::Vertical, Along::Horizontal] {
            let (near, far, from, to) = match along {
                Along::Vertical => (
                    one.loc.x + one.size.w,
                    other.loc.x,
                    one.loc.y.max(other.loc.y),
                    (one.loc.y + one.size.h).min(other.loc.y + other.size.h),
                ),
                Along::Horizontal => (
                    one.loc.y + one.size.h,
                    other.loc.y,
                    one.loc.x.max(other.loc.x),
                    (one.loc.x + one.size.w).min(other.loc.x + other.size.w),
                ),
            };
            if (near - far).abs() > TOUCH || from + OVERLAP > to {
                continue;
            }
            let divider = Divider {
                along,
                at: (near + far) / 2,
                from,
                to,
            };
            match dividers
                .iter_mut()
                .find(|other| other.along == along && (other.at - divider.at).abs() <= TOUCH)
            {
                Some(other) => {
                    other.from = other.from.min(divider.from);
                    other.to = other.to.max(divider.to);
                }
                None => dividers.push(divider),
            }
        }
    }
    dividers
}

fn pairs(
    rects: &[Rect<i32, Logical>],
) -> impl Iterator<Item = (Rect<i32, Logical>, Rect<i32, Logical>)> {
    rects.iter().enumerate().flat_map(|(index, one)| {
        rects
            .iter()
            .skip(index + 1)
            .flat_map(move |other| [(*one, *other), (*other, *one)])
    })
}
