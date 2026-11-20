use rikka_compositor_geometry::{Logical, Point, Rect, Region, Size};

use crate::SLACK;
use crate::shape::{Radii, Shape, Step, cut};

pub(super) fn grown(whole: &Region<Logical>, open: &Region<Logical>, seed: Step) -> Shape {
    let (start, end) = (seed.loc.x, seed.loc.x + seed.size.w);
    let column: Vec<(i32, i32)> = (seed.loc.y..)
        .map_while(|y| whole.span_at((start + (end - start) / 2, y)))
        .map(|span| (span.loc.x, span.loc.x + span.size.w))
        .collect();
    let read = |lead| Grown::new(whole, open, seed, &column, lead).shape();
    [read(1), read(0)]
        .into_iter()
        .flatten()
        .max_by_key(Shape::area)
        .unwrap_or(Shape {
            left: start,
            top: seed.loc.y,
            right: end,
            bottom: seed.loc.y + seed.size.h,
            radii: Radii::SQUARE,
        })
}

struct Corner {
    fitting: Vec<i32>,
    traced: Vec<i32>,
}

impl Corner {
    fn shared_radii(near: &Corner, far: &Corner) -> (i32, i32) {
        let shared = near
            .traced
            .iter()
            .rev()
            .find(|radius| far.traced.contains(radius));
        match (near.traced.last(), far.traced.last(), shared) {
            (_, _, Some(&shared)) => (shared, shared),
            (Some(&near), Some(&far), None) => (near, far),
            (Some(&seen), None, None) => (seen, far.clamp_radius(seen)),
            (None, Some(&seen), None) => (near.clamp_radius(seen), seen),
            (None, None, None) => (0, 0),
        }
    }

    fn clamp_radius(&self, radius: i32) -> i32 {
        self.fitting
            .iter()
            .rev()
            .copied()
            .find(|&fitting| fitting <= radius)
            .unwrap_or(0)
    }
}
#[derive(Debug, Clone, Copy)]
struct Bottom {
    left: i32,
    right: i32,
    last: i32,
}

impl Bottom {
    const FLAT: Bottom = Bottom {
        left: 0,
        right: 0,
        last: 0,
    };
}

#[derive(Debug, Clone, Copy)]
struct Rounding {
    radius: i32,
    seen: bool,
}

struct Side {
    edges: Vec<i32>,
    anchor: i32,
    mirrored: bool,
    lead: i32,
}

impl Side {
    fn rows(&self) -> i32 {
        self.edges.len() as i32 + self.lead
    }

    fn real(&self, index: i32) -> bool {
        (self.lead..self.rows()).contains(&index)
    }

    fn edge(&self, index: i32) -> i32 {
        self.edges[(index - self.lead) as usize]
    }

    fn outer(&self, radius: i32) -> i32 {
        self.anchor - cut(radius, self.lead)
    }

    fn corner(&self) -> Corner {
        let fitting: Vec<i32> = (self.lead..=(self.rows() + 1) / 2)
            .filter(|&radius| self.fits(radius))
            .collect();
        Corner {
            traced: fitting
                .iter()
                .copied()
                .filter(|&radius| self.traces(radius))
                .collect(),
            fitting,
        }
    }

    fn fits(&self, radius: i32) -> bool {
        let (outer, rows) = (self.outer(radius), self.rows());
        (self.lead..radius).all(|index| self.edge(index) <= outer + cut(radius, index))
            && (radius >= rows || self.edge(radius) <= outer + cut(radius, rows - 1 - radius))
    }

    fn traces(&self, radius: i32) -> bool {
        let outer = self.outer(radius);
        let first = self.lead + 1;
        let mut exact = false;
        for index in first..=radius.max(first).min(self.rows() - 1) {
            let (edge, inner) = (self.edge(index), outer + cut(radius, index));
            if edge >= inner - SLACK {
                exact = true;
            } else if edge >= outer {
                return false;
            }
        }
        exact
    }
}

struct Edge {
    side: Side,
    outer: i32,
    radius: i32,
}

impl Edge {
    fn new(side: Side, radius: i32) -> Edge {
        Edge {
            outer: side.outer(radius),
            side,
            radius,
        }
    }

    fn hidden(&self, index: i32) -> bool {
        self.side.edge(index) < self.outer
    }

    fn inner(&self, index: i32, below: i32, last: i32) -> i32 {
        self.outer + cut(self.radius, index).max(cut(below, last - index))
    }

    fn bottom(&self, last: i32) -> Option<Rounding> {
        let rows_from = |from: i32| (from.max(0)..=last).filter(|&index| self.side.real(index));
        let contains = |radius: i32| {
            rows_from(last + 1 - radius.max(1))
                .all(|index| self.side.edge(index) <= self.inner(index, radius, last))
        };
        let most = (last + 1) / 2;
        if !contains(most) {
            return None;
        }
        let radius = smallest(most, contains);
        let seen = rows_from(last - radius).any(|index| {
            let edge = self.side.edge(index);
            edge >= self.outer && edge >= self.inner(index, radius, last) - SLACK
        });
        Some(Rounding { radius, seen })
    }

    fn holds(&self, grown: &Grown, index: i32, inner: i32, below: Option<i32>) -> bool {
        let edge = self.side.edge(index);
        if edge > inner {
            return false;
        }
        if edge < self.outer {
            return true;
        }
        let (from, to) = match self.side.mirrored {
            false => (edge, inner - SLACK),
            true => (SLACK - inner, -edge),
        };
        if to <= from {
            return true;
        }
        let row = |y| Rect::new(Point::new(from, y), Size::new(to - from, 1));
        !grown.open.overlaps(row(grown.top() + index))
            || below.is_some_and(|below| grown.whole.covers(row(below)))
    }
}

struct Grown<'a> {
    left: Edge,
    right: Edge,
    whole: &'a Region<Logical>,
    open: &'a Region<Logical>,
    seed: Step,
    lead: i32,
}

impl<'a> Grown<'a> {
    fn new(
        whole: &'a Region<Logical>,
        open: &'a Region<Logical>,
        seed: Step,
        column: &[(i32, i32)],
        lead: i32,
    ) -> Self {
        let left = Side {
            edges: column.iter().map(|&(left, _)| left).collect(),
            anchor: seed.loc.x,
            mirrored: false,
            lead,
        };
        let right = Side {
            edges: column.iter().map(|&(_, right)| -right).collect(),
            anchor: -(seed.loc.x + seed.size.w),
            mirrored: true,
            lead,
        };
        let (top_left, top_right) = Corner::shared_radii(&left.corner(), &right.corner());
        Grown {
            left: Edge::new(left, top_left),
            right: Edge::new(right, top_right),
            whole,
            open,
            seed,
            lead,
        }
    }

    fn top(&self) -> i32 {
        self.seed.loc.y - self.lead
    }

    fn shape(&self) -> Option<Shape> {
        let rows = self.left.side.rows();
        let reach = (self.lead + 1..rows)
            .find(|&index| self.left.hidden(index) && self.right.hidden(index))
            .unwrap_or(rows + 1);
        let straight = (self.lead..reach.min(rows))
            .find(|&index| !self.holds(index, Bottom::FLAT, None))
            .unwrap_or(reach.min(rows));
        let width = -self.right.outer - self.left.outer;
        let deepest = reach.min(2 * straight).min(straight + width / 2);
        (straight + 1..=deepest)
            .rev()
            .find_map(|height| self.rounded(height, straight))
            .or_else(|| self.shaped(straight.max(self.lead + 1), Bottom::FLAT))
    }

    fn rounded(&self, height: i32, straight: i32) -> Option<Shape> {
        let last = height - 1;
        let near = self.left.bottom(last)?;
        let far = self.right.bottom(last)?;
        let (left, right) = match (near.seen, far.seen) {
            (true, true) => (near.radius, far.radius),
            (true, false) => (near.radius, far.radius.max(near.radius)),
            (false, true) => (near.radius.max(far.radius), far.radius),
            (false, false) => (
                near.radius.max(self.left.radius),
                far.radius.max(self.right.radius),
            ),
        };
        let least = height - straight;
        let (left, right) = match left.max(right) < least {
            true => (left.max(least), right.max(least)),
            false => (left, right),
        };
        let deepest = left.max(right);
        let from = height - deepest;
        let seed = last - self.lead;
        let keeps_seed = cut(left, seed) <= cut(self.left.radius, self.lead)
            && cut(right, seed) <= cut(self.right.radius, self.lead);
        if 2 * deepest > height || !keeps_seed {
            return None;
        }
        let bottom = Bottom { left, right, last };
        let below = Some(self.top() + height);
        (from.min(last)..height)
            .all(|index| self.holds(index, bottom, below))
            .then(|| self.shaped(height, bottom))
            .flatten()
    }

    fn holds(&self, index: i32, bottom: Bottom, below: Option<i32>) -> bool {
        let near = self.left.inner(index, bottom.left, bottom.last);
        let far = self.right.inner(index, bottom.right, bottom.last);
        if !self.left.side.real(index) {
            return near >= -far;
        }
        near < -far
            && self.left.holds(self, index, near, below)
            && self.right.holds(self, index, far, below)
    }

    fn shaped(&self, height: i32, bottom: Bottom) -> Option<Shape> {
        let shape = Shape {
            left: self.left.outer,
            top: self.top(),
            right: -self.right.outer,
            bottom: self.top() + height,
            radii: Radii {
                top_left: self.left.radius,
                top_right: self.right.radius,
                bottom_right: bottom.right,
                bottom_left: bottom.left,
            },
        };
        let opens_on_nothing =
            self.lead == 0 || self.left.radius + self.right.radius >= shape.width();
        (opens_on_nothing && shape.rounds_within_itself()).then_some(shape)
    }
}

fn smallest(mut holding: i32, holds: impl Fn(i32) -> bool) -> i32 {
    let mut failing = -1;
    while holding - failing > 1 {
        let tried = failing + (holding - failing) / 2;
        match holds(tried) {
            true => holding = tried,
            false => failing = tried,
        }
    }
    holding
}
