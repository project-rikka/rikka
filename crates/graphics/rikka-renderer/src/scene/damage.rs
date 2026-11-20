use rikka_types::Rect;

use crate::scene::audience::{Audience, Viewer};
use crate::scene::region::Region;

const MAX_RECTS: usize = 16;

const ABSENT_AFTER_SCREEN_DRAWS: u32 = 600;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum DamagePolicy {
    #[default]
    Merged,
    Exact,
}

#[derive(Debug, Default)]
pub(crate) struct DamageTracker {
    policy: DamagePolicy,
    full: bool,
    rects: Vec<Rect>,
    region: Region,
    clipped: Vec<Rect>,
    apart: Vec<Rect>,
}

impl DamageTracker {
    pub(crate) fn new(policy: DamagePolicy) -> Self {
        Self {
            policy,
            full: false,
            rects: Vec::new(),
            region: Region::default(),
            clipped: Vec::new(),
            apart: Vec::new(),
        }
    }

    pub(crate) fn policy(&self) -> DamagePolicy {
        self.policy
    }
    pub(crate) fn set_policy(&mut self, policy: DamagePolicy) {
        self.policy = policy;
    }

    pub(crate) fn add(&mut self, rect: Rect) {
        if self.full || rect.is_empty() {
            return;
        }
        if self.policy == DamagePolicy::Exact {
            if self
                .rects
                .last()
                .is_some_and(|last| last.contains_rect(&rect))
            {
                return;
            }
            self.rects.push(rect);
            return;
        }
        self.merge_rect(rect);
        if self.rects.len() > MAX_RECTS
            && let Some(newest) = self.rects.pop()
        {
            let partner = self
                .rects
                .swap_remove(cheapest_partner(&self.rects, &newest));
            self.merge_rect(newest.union(&partner));
        }
    }

    fn merge_rect(&mut self, rect: Rect) {
        if self.rects.iter().any(|other| other.contains_rect(&rect)) {
            return;
        }
        let mut merged = rect;
        while let Some(index) = self
            .rects
            .iter()
            .position(|other| other.intersects(&merged.inflate(1.0)))
        {
            merged = merged.union(&self.rects.swap_remove(index));
        }
        self.rects.push(merged);
    }

    pub(crate) fn add_all(&mut self, rects: impl IntoIterator<Item = Rect>) {
        for rect in rects {
            self.add(rect);
        }
    }

    pub(crate) fn add_tracked(&mut self, other: &DamageTracker) {
        match other.full {
            true => self.mark_full(),
            false => self.add_all(other.rects.iter().copied()),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.full = false;
        self.rects.clear();
    }

    pub(crate) fn mark_full(&mut self) {
        self.full = true;
        self.rects.clear();
    }

    pub(crate) fn is_empty(&self) -> bool {
        !self.full && self.rects.is_empty()
    }

    pub(crate) fn covers(&mut self, viewport: Rect) -> bool {
        if self.full {
            return true;
        }
        let Self {
            rects,
            region,
            clipped,
            apart,
            ..
        } = self;
        clipped.clear();
        clipped.extend(rects.iter().filter_map(|rect| rect.intersection(&viewport)));
        region.build(clipped, apart);
        let covered: f64 = apart
            .iter()
            .map(|rect| f64::from(rect.width()) * f64::from(rect.height()))
            .sum();
        covered >= f64::from(viewport.width()) * f64::from(viewport.height()) - 0.5
    }

    pub(crate) fn transfer_into(&mut self, into: &mut PendingDamage) {
        into.rects.clear();
        into.full = std::mem::take(&mut self.full);
        std::mem::swap(&mut into.rects, &mut self.rects);
    }

    pub(crate) fn receive(&mut self, handed: &mut PendingDamage) {
        match std::mem::take(&mut handed.full) {
            true => {
                self.mark_full();
                handed.rects.clear();
            }
            false => self.add_all(handed.rects.drain(..)),
        }
    }

    pub(crate) fn take_into(&mut self, viewport: Rect, out: &mut Vec<Rect>) {
        out.clear();
        if self.full {
            self.full = false;
            self.rects.clear();
            out.push(viewport);
            return;
        }
        let Self {
            policy,
            rects,
            region,
            clipped,
            ..
        } = self;
        match policy {
            DamagePolicy::Merged => out.extend(
                rects
                    .drain(..)
                    .filter_map(|rect| rect.intersection(&viewport)),
            ),
            DamagePolicy::Exact => {
                clipped.clear();
                clipped.extend(
                    rects
                        .drain(..)
                        .filter_map(|rect| rect.round_out().intersection(&viewport)),
                );
                region.build(clipped, out);
            }
        }
    }
}
#[derive(Debug, Default)]
pub(crate) struct PendingDamage {
    full: bool,
    rects: Vec<Rect>,
}

impl PendingDamage {
    pub(crate) fn is_empty(&self) -> bool {
        !self.full && self.rects.is_empty()
    }
}

#[derive(Debug)]
pub(crate) struct ViewerDamage {
    viewers: [DamageTracker; Viewer::ALL.len()],
    watched: [bool; Viewer::ALL.len()],
    unseen: [u32; Viewer::ALL.len()],
    dirty_unwatched: [bool; Viewer::ALL.len()],
}

impl ViewerDamage {
    pub(crate) fn new(policy: DamagePolicy) -> Self {
        let mut watched = [false; Viewer::ALL.len()];
        watched[Viewer::Screen.index()] = true;
        Self {
            viewers: Viewer::ALL.map(|_| DamageTracker::new(policy)),
            watched,
            unseen: [0; Viewer::ALL.len()],
            dirty_unwatched: [false; Viewer::ALL.len()],
        }
    }

    pub(crate) fn policy(&self) -> DamagePolicy {
        self.viewers[Viewer::Screen.index()].policy()
    }

    pub(crate) fn set_policy(&mut self, policy: DamagePolicy) {
        for tracker in &mut self.viewers {
            tracker.set_policy(policy);
        }
    }

    pub(crate) fn add(&mut self, rect: Rect) {
        if rect.is_empty() {
            return;
        }
        for (index, tracker) in self.viewers.iter_mut().enumerate() {
            if self.watched[index] {
                tracker.add(rect);
            } else {
                self.dirty_unwatched[index] = true;
            }
        }
    }

    pub(crate) fn add_for(&mut self, audience: Audience, rect: Rect) {
        if rect.is_empty() {
            return;
        }
        for viewer in Viewer::ALL {
            if !audience.admits(viewer) {
                continue;
            }
            let index = viewer.index();
            if self.watched[index] {
                self.viewers[index].add(rect);
            } else {
                self.dirty_unwatched[index] = true;
            }
        }
    }

    pub(crate) fn add_all(&mut self, rects: impl IntoIterator<Item = Rect>) {
        for rect in rects {
            self.add(rect);
        }
    }

    pub(crate) fn reset(&mut self) {
        for tracker in &mut self.viewers {
            tracker.reset();
        }
        self.dirty_unwatched = [false; Viewer::ALL.len()];
    }

    pub(crate) fn mark_full(&mut self) {
        for (index, tracker) in self.viewers.iter_mut().enumerate() {
            if self.watched[index] {
                tracker.mark_full();
            } else {
                self.dirty_unwatched[index] = true;
            }
        }
    }

    pub(crate) fn mark_full_for(&mut self, viewer: Viewer) {
        let index = viewer.index();
        self.viewers[index].mark_full();
        if !self.watched[index] {
            self.dirty_unwatched[index] = true;
        }
    }

    pub(crate) fn is_empty_for(&self, viewer: Viewer) -> bool {
        self.watched[viewer.index()] && self.viewers[viewer.index()].is_empty()
    }

    pub(crate) fn covers(&mut self, viewer: Viewer, viewport: Rect) -> bool {
        !self.watched[viewer.index()] || self.viewers[viewer.index()].covers(viewport)
    }

    pub(crate) fn take_into(&mut self, viewer: Viewer, viewport: Rect, out: &mut Vec<Rect>) {
        let index = viewer.index();
        if !std::mem::replace(&mut self.watched[index], true) {
            self.viewers[index].mark_full();
        }
        self.unseen[index] = 0;
        if viewer == Viewer::Screen {
            self.forget_absent_viewers();
        }
        self.viewers[index].take_into(viewport, out);
    }

    fn forget_absent_viewers(&mut self) {
        for viewer in Viewer::ALL {
            let index = viewer.index();
            if viewer == Viewer::Screen || !self.watched[index] {
                continue;
            }
            self.unseen[index] += 1;
            if self.unseen[index] > ABSENT_AFTER_SCREEN_DRAWS {
                self.watched[index] = false;
                self.viewers[index].reset();
            }
        }
    }

    pub(crate) fn transfer_into(&mut self, into: &mut ViewerPending) {
        for (index, ((tracker, watched), pending)) in self
            .viewers
            .iter_mut()
            .zip(self.watched)
            .zip(&mut into.viewers)
            .enumerate()
        {
            if !watched && std::mem::take(&mut self.dirty_unwatched[index]) {
                tracker.mark_full();
            }
            tracker.transfer_into(pending);
        }
    }

    pub(crate) fn receive(&mut self, handed: &mut ViewerPending) {
        for ((tracker, watched), pending) in self
            .viewers
            .iter_mut()
            .zip(self.watched)
            .zip(&mut handed.viewers)
        {
            match watched {
                true => tracker.receive(pending),
                false => {
                    pending.full = false;
                    pending.rects.clear();
                }
            }
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ViewerPending {
    viewers: [PendingDamage; Viewer::ALL.len()],
}

impl ViewerPending {
    pub(crate) fn is_empty(&self) -> bool {
        self.viewers.iter().all(PendingDamage::is_empty)
    }
}

fn cheapest_partner(rects: &[Rect], newest: &Rect) -> usize {
    let area = |rect: &Rect| f64::from(rect.width()) * f64::from(rect.height());
    let mut best = (0, f64::INFINITY);
    for (index, rect) in rects.iter().enumerate() {
        let grown = area(&rect.union(newest)) - area(rect) - area(newest);
        if grown < best.1 {
            best = (index, grown);
        }
    }
    best.0
}
