use rikka_compositor_geometry::{Logical, Point, Rect, Size};

use crate::edges::Edges;
use crate::gaps::Gaps;

const MIN_SHARE: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    LeftRight,
    TopBottom,
}

#[derive(Debug)]
pub struct Tree<T>(Option<Node<T>>);

#[derive(Debug)]
enum Node<T> {
    Leaf(T),
    Split {
        axis: Axis,
        share: f64,
        first: Box<Node<T>>,
        second: Box<Node<T>>,
    },
}

impl<T> Default for Tree<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T: Clone + PartialEq> Tree<T> {
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    pub fn contains(&self, window: &T) -> bool {
        self.0.as_ref().is_some_and(|root| root.contains(window))
    }

    pub fn windows(&self) -> Vec<T> {
        let mut into = Vec::new();
        if let Some(root) = &self.0 {
            root.windows(&mut into);
        }
        into
    }

    pub fn insert(&mut self, area: Rect<i32, Logical>, window: T, next_to: Option<&T>) {
        let Some(root) = &mut self.0 else {
            self.0 = Some(Node::Leaf(window));
            return;
        };
        let target = root.target(next_to).clone();
        let axis = longer(root.rect_of(area, &target).unwrap_or(area));
        root.split(&target, window, axis);
    }

    pub fn arrival(
        &self,
        area: Rect<i32, Logical>,
        gaps: Gaps,
        next_to: Option<&T>,
    ) -> Rect<i32, Logical> {
        let partitioned = gaps.partitioned(area);
        let Some(root) = &self.0 else {
            return gaps.inset(partitioned);
        };
        let target = root.target(next_to);
        let axis = longer(root.rect_of(area, target).unwrap_or(area));
        let tile = root.rect_of(partitioned, target).unwrap_or(partitioned);
        let (_, second) = halves(axis, 0.5, tile);
        gaps.inset(second)
    }

    pub fn remove(&mut self, window: &T) -> bool {
        if !self.contains(window) {
            return false;
        }
        self.0 = self.0.take().and_then(|root| root.without(window));
        true
    }

    pub fn tiles(&self, area: Rect<i32, Logical>, gaps: Gaps) -> Vec<(T, Rect<i32, Logical>)> {
        let mut into = Vec::new();
        if let Some(root) = &self.0 {
            root.tiles(gaps.partitioned(area), gaps, &mut into);
        }
        into
    }
}

impl<T: Clone + PartialEq> Tree<T> {
    pub fn swap(&mut self, one: &T, other: &T) -> bool {
        if one == other || !self.contains(one) || !self.contains(other) {
            return false;
        }
        if let Some(root) = &mut self.0 {
            root.swap(one, other);
        }
        true
    }

    pub fn resize(
        &mut self,
        area: Rect<i32, Logical>,
        gaps: Gaps,
        window: &T,
        edges: Edges,
        delta: Point<f64, Logical>,
    ) -> bool {
        let Some(root) = &mut self.0 else {
            return false;
        };
        let area = gaps.partitioned(area);
        let mut moved = false;
        if edges.contains(Edges::LEFT) {
            moved |= root.drag(area, window, Axis::LeftRight, false, delta.x);
        } else if edges.contains(Edges::RIGHT) {
            moved |= root.drag(area, window, Axis::LeftRight, true, delta.x);
        }
        if edges.contains(Edges::TOP) {
            moved |= root.drag(area, window, Axis::TopBottom, false, delta.y);
        } else if edges.contains(Edges::BOTTOM) {
            moved |= root.drag(area, window, Axis::TopBottom, true, delta.y);
        }
        moved
    }
}

fn longer(tile: Rect<i32, Logical>) -> Axis {
    if tile.size.w >= tile.size.h {
        Axis::LeftRight
    } else {
        Axis::TopBottom
    }
}

impl<T: Clone + PartialEq> Node<T> {
    fn target<'a>(&'a self, next_to: Option<&'a T>) -> &'a T {
        next_to
            .filter(|next_to| self.contains(next_to))
            .unwrap_or_else(|| self.last())
    }

    fn contains(&self, window: &T) -> bool {
        match self {
            Self::Leaf(leaf) => leaf == window,
            Self::Split { first, second, .. } => first.contains(window) || second.contains(window),
        }
    }

    fn windows(&self, into: &mut Vec<T>) {
        match self {
            Self::Leaf(leaf) => into.push(leaf.clone()),
            Self::Split { first, second, .. } => {
                first.windows(into);
                second.windows(into);
            }
        }
    }

    fn last(&self) -> &T {
        match self {
            Self::Leaf(leaf) => leaf,
            Self::Split { second, .. } => second.last(),
        }
    }

    fn rect_of(&self, area: Rect<i32, Logical>, window: &T) -> Option<Rect<i32, Logical>> {
        match self {
            Self::Leaf(leaf) => (leaf == window).then_some(area),
            Self::Split {
                axis,
                share,
                first,
                second,
            } => {
                let (a, b) = halves(*axis, *share, area);
                first
                    .rect_of(a, window)
                    .or_else(|| second.rect_of(b, window))
            }
        }
    }

    fn tiles(&self, area: Rect<i32, Logical>, gaps: Gaps, into: &mut Vec<(T, Rect<i32, Logical>)>) {
        match self {
            Self::Leaf(leaf) => into.push((leaf.clone(), gaps.inset(area))),
            Self::Split {
                axis,
                share,
                first,
                second,
            } => {
                let (a, b) = halves(*axis, *share, area);
                first.tiles(a, gaps, into);
                second.tiles(b, gaps, into);
            }
        }
    }

    fn split(&mut self, target: &T, window: T, axis: Axis) -> bool {
        match self {
            Self::Leaf(leaf) => {
                if leaf != target {
                    return false;
                }
                let first = std::mem::replace(self, Self::Leaf(window.clone()));
                *self = Self::Split {
                    axis,
                    share: 0.5,
                    first: Box::new(first),
                    second: Box::new(Self::Leaf(window)),
                };
                true
            }
            Self::Split { first, second, .. } => {
                first.split(target, window.clone(), axis) || second.split(target, window, axis)
            }
        }
    }

    fn without(self, window: &T) -> Option<Self> {
        let Self::Split {
            axis,
            share,
            first,
            second,
        } = self
        else {
            return None;
        };
        let (first, second) = if first.contains(window) {
            match first.without(window) {
                Some(first) => (Box::new(first), second),
                None => return Some(*second),
            }
        } else {
            match second.without(window) {
                Some(second) => (first, Box::new(second)),
                None => return Some(*first),
            }
        };
        Some(Self::Split {
            axis,
            share,
            first,
            second,
        })
    }

    fn swap(&mut self, one: &T, other: &T) {
        match self {
            Self::Leaf(leaf) => {
                if leaf == one {
                    *leaf = other.clone();
                } else if leaf == other {
                    *leaf = one.clone();
                }
            }
            Self::Split { first, second, .. } => {
                first.swap(one, other);
                second.swap(one, other);
            }
        }
    }

    fn drag(
        &mut self,
        area: Rect<i32, Logical>,
        window: &T,
        axis: Axis,
        in_first: bool,
        delta: f64,
    ) -> bool {
        let Self::Split {
            axis: own,
            share,
            first,
            second,
        } = self
        else {
            return false;
        };
        let (a, b) = halves(*own, *share, area);
        let here = if first.contains(window) {
            true
        } else if second.contains(window) {
            false
        } else {
            return false;
        };
        let (child, child_area) = if here { (first, a) } else { (second, b) };
        if child.drag(child_area, window, axis, in_first, delta) {
            return true;
        }
        if *own != axis || here != in_first {
            return false;
        }
        let span = f64::from(match axis {
            Axis::LeftRight => area.size.w,
            Axis::TopBottom => area.size.h,
        });
        if span <= 0.0 {
            return false;
        }
        let moved = (*share + delta / span).clamp(MIN_SHARE, 1.0 - MIN_SHARE);
        let changed = moved != *share;
        *share = moved;
        changed
    }
}

fn halves(
    axis: Axis,
    share: f64,
    area: Rect<i32, Logical>,
) -> (Rect<i32, Logical>, Rect<i32, Logical>) {
    match axis {
        Axis::LeftRight => {
            let width = (f64::from(area.size.w) * share).round() as i32;
            let width = width.clamp(0, area.size.w);
            (
                Rect::new(area.loc, Size::new(width, area.size.h)),
                Rect::new(
                    area.loc + Point::new(width, 0),
                    Size::new(area.size.w - width, area.size.h),
                ),
            )
        }
        Axis::TopBottom => {
            let height = (f64::from(area.size.h) * share).round() as i32;
            let height = height.clamp(0, area.size.h);
            (
                Rect::new(area.loc, Size::new(area.size.w, height)),
                Rect::new(
                    area.loc + Point::new(0, height),
                    Size::new(area.size.w, area.size.h - height),
                ),
            )
        }
    }
}
