use std::fmt;

use crate::{Point, Rect};

const MAX_RECTS: usize = 32;

pub struct Damage<S> {
    rects: Vec<Rect<i32, S>>,
}

impl<S> Damage<S> {
    pub fn new() -> Self {
        Damage { rects: Vec::new() }
    }

    pub fn add(&mut self, rect: Rect<i32, S>) {
        if rect.is_empty() {
            return;
        }
        let mut merged = rect;
        while let Some(index) = self
            .rects
            .iter()
            .position(|other| worth_joining(*other, merged))
        {
            merged = merged.merge(self.rects.swap_remove(index));
        }
        self.rects.push(merged);
        if self.rects.len() > MAX_RECTS {
            let whole = self.bounding_box();
            self.rects.clear();
            self.rects.push(whole);
        }
    }

    pub fn clear(&mut self) {
        self.rects.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.rects.is_empty()
    }

    pub fn bounding_box(&self) -> Rect<i32, S> {
        self.rects
            .iter()
            .copied()
            .reduce(Rect::merge)
            .unwrap_or_default()
    }

    pub fn translate(&mut self, offset: Point<i32, S>) {
        for rect in &mut self.rects {
            rect.loc += offset;
        }
    }

    pub fn intersect(&self, area: Rect<i32, S>) -> Self {
        Damage {
            rects: self
                .rects
                .iter()
                .filter_map(|rect| rect.intersection(area))
                .collect(),
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = Rect<i32, S>> + '_ {
        self.rects.iter().copied()
    }
}

fn worth_joining<S>(a: Rect<i32, S>, b: Rect<i32, S>) -> bool {
    if !a.overlaps_or_touches(b) {
        return false;
    }
    let shared = a.intersection(b).map_or(0, area);
    let covered = area(a) + area(b) - shared;
    area(a.merge(b)) * 2 <= covered * 3
}

fn area<S>(rect: Rect<i32, S>) -> i64 {
    i64::from(rect.size.w) * i64::from(rect.size.h)
}

impl<S> Extend<Rect<i32, S>> for Damage<S> {
    fn extend<I: IntoIterator<Item = Rect<i32, S>>>(&mut self, rects: I) {
        for rect in rects {
            self.add(rect);
        }
    }
}

impl<S> FromIterator<Rect<i32, S>> for Damage<S> {
    fn from_iter<I: IntoIterator<Item = Rect<i32, S>>>(rects: I) -> Self {
        let mut damage = Damage::new();
        damage.extend(rects);
        damage
    }
}

impl<S> Default for Damage<S> {
    fn default() -> Self {
        Damage::new()
    }
}

impl<S> Clone for Damage<S> {
    fn clone(&self) -> Self {
        Damage {
            rects: self.rects.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.rects.clone_from(&source.rects);
    }
}

impl<S> fmt::Debug for Damage<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(&self.rects).finish()
    }
}
