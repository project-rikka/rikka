use std::collections::HashMap;

use rikka_compositor_geometry::{Physical, Rect};
use rikka_types::{Lens, RoundedRect};
use std::hash::Hash;

use super::placed::{Look, Pane};

const AT_MOST: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Layer<K> {
    Surface(K),
    Pane(u64),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Occupant<'a, K> {
    Surface {
        surface: K,
        rect: Rect<i32, Physical>,
        alpha: f32,
        clip: Option<RoundedRect>,
        changed: &'a [Rect<i32, Physical>],
    },
    Pane(&'a Pane),
}

impl<K: Copy> Occupant<'_, K> {
    fn layer(&self) -> Layer<K> {
        match self {
            Occupant::Surface { surface, .. } => Layer::Surface(*surface),
            Occupant::Pane(pane) => Layer::Pane(pane.key),
        }
    }

    fn geometry(&self) -> Rect<i32, Physical> {
        match self {
            Occupant::Surface { rect, .. } => *rect,
            Occupant::Pane(pane) => pane.geometry(),
        }
    }

    fn alpha(&self) -> f32 {
        match self {
            Occupant::Surface { alpha, .. } => *alpha,
            Occupant::Pane(pane) => pane.alpha,
        }
    }

    fn clip(&self) -> Option<RoundedRect> {
        match self {
            Occupant::Surface { clip, .. } => *clip,
            Occupant::Pane(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PaneState {
    geometry: Rect<i32, Physical>,
    alpha: f32,
    clip: Option<RoundedRect>,
    depth: usize,
    reordered: bool,
    look: Option<Look>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StaleRegion {
    pub(crate) key: u64,
    pub(crate) stale: Vec<Rect<i32, Physical>>,
}

#[derive(Debug)]
pub(crate) struct Repaint<K> {
    current: HashMap<Layer<K>, PaneState>,
    seen: HashMap<Layer<K>, PaneState>,
    unlanded: HashMap<K, Vec<Rect<i32, Physical>>>,
    changed: Vec<Rect<i32, Physical>>,
    left: Vec<(Rect<i32, Physical>, usize)>,
    bands: Vec<Rect<i32, Physical>>,
}

impl<K> Default for Repaint<K> {
    fn default() -> Self {
        Self {
            current: HashMap::new(),
            seen: HashMap::new(),
            unlanded: HashMap::new(),
            changed: Vec::new(),
            left: Vec::new(),
            bands: Vec::new(),
        }
    }
}

impl<K: Copy + Eq + Hash> Repaint<K> {
    pub(crate) fn stale_regions(&mut self, painted: &[Occupant<'_, K>]) -> Vec<StaleRegion> {
        for stood in painted {
            if let Occupant::Surface {
                surface, changed, ..
            } = stood
                && !changed.is_empty()
            {
                self.unlanded
                    .entry(*surface)
                    .or_default()
                    .extend_from_slice(changed);
            }
        }
        if !painted
            .iter()
            .any(|stood| matches!(stood, Occupant::Pane(_)))
        {
            self.current.clear();
            self.seen.clear();
            return Vec::new();
        }

        self.seen.clear();
        let mut deepest = 0;
        for (depth, stood) in painted.iter().rev().enumerate() {
            let was = self.current.get(&stood.layer());
            let reordered = was.is_some_and(|was| was.depth < deepest);
            if let Some(was) = was.filter(|_| !reordered) {
                deepest = was.depth;
            }
            self.seen.insert(
                stood.layer(),
                PaneState {
                    geometry: stood.geometry(),
                    alpha: stood.alpha(),
                    clip: stood.clip(),
                    depth,
                    reordered,
                    look: match stood {
                        Occupant::Pane(pane) => Some(pane.look()),
                        Occupant::Surface { .. } => None,
                    },
                },
            );
        }

        self.left.clear();
        self.left.extend(
            self.current
                .iter()
                .filter(|(layer, _)| !self.seen.contains_key(*layer))
                .map(|(_, was)| (was.geometry, was.depth)),
        );

        self.bands.clear();
        self.bands
            .extend(painted.iter().filter_map(|stood| match stood {
                Occupant::Pane(pane) => Some(pane.watched()),
                Occupant::Surface { .. } => None,
            }));

        let mut owed = Vec::new();
        self.changed.clear();
        for stood in painted {
            let now = self.seen[&stood.layer()];
            let was = self.current.get(&stood.layer()).copied();
            match stood {
                Occupant::Pane(pane) => {
                    owed.push(self.stale_region(pane, now, was));
                }
                Occupant::Surface {
                    surface,
                    rect,
                    alpha,
                    clip,
                    ..
                } => {
                    let bands = &self.bands;
                    let behind_glass =
                        |rect: Rect<i32, Physical>| bands.iter().any(|band| rect.overlaps(*band));
                    if !behind_glass(*rect) && !was.is_some_and(|was| behind_glass(was.geometry)) {
                        continue;
                    }
                    match was {
                        Some(was)
                            if was.geometry == *rect
                                && was.alpha == *alpha
                                && was.clip == *clip
                                && !now.reordered =>
                        {
                            if let Some(changed) = self.unlanded.get(surface) {
                                self.changed.extend_from_slice(changed);
                            }
                        }
                        Some(was) => self.changed.extend([was.geometry, *rect]),
                        None => self.changed.push(*rect),
                    }
                }
            }
        }
        owed
    }

    fn stale_region(&mut self, pane: &Pane, now: PaneState, was: Option<PaneState>) -> StaleRegion {
        let watched = pane.watched();
        let recut = was.is_some_and(|was| was.look != now.look);
        let mut stale = Vec::new();
        match was.filter(|was| filter_of(was) == filter_of(&now) && !now.reordered) {
            Some(was) => {
                let exposed = watched.subtract_rect(was.geometry.grown(pane.reads()));
                let behind = self
                    .left
                    .iter()
                    .filter(|(_, depth)| *depth > was.depth)
                    .map(|(rect, _)| rect)
                    .chain(self.changed.iter());
                for rect in exposed.iter().chain(behind) {
                    pane.touched_by(*rect, &mut stale);
                }
            }
            None => stale.push(watched),
        }
        coalesce(&mut stale);
        match was {
            Some(was) if was.geometry != now.geometry => {
                self.changed.extend([was.geometry, now.geometry]);
            }
            Some(was) if was.alpha != now.alpha => {
                self.changed.push(now.geometry);
            }
            Some(was) if recut => {
                let mut reshaped = Vec::new();
                let corners = was
                    .look
                    .is_some_and(|look| pane.reshaped_from(look, &mut reshaped));
                match corners {
                    true => self.changed.extend(
                        reshaped
                            .iter()
                            .filter_map(|rect| rect.intersection(now.geometry)),
                    ),
                    false => self.changed.push(now.geometry),
                }
            }
            _ if !stale.is_empty() => self.changed.extend(
                stale
                    .iter()
                    .filter_map(|rect| rect.intersection(now.geometry)),
            ),
            _ => {}
        }
        StaleRegion {
            key: pane.key,
            stale,
        }
    }

    pub(crate) fn landed(&mut self) {
        std::mem::swap(&mut self.current, &mut self.seen);
        self.unlanded.clear();
    }
}

fn coalesce(rects: &mut Vec<Rect<i32, Physical>>) {
    rects.retain(|rect| !rect.is_empty());
    let mut merged: Vec<Rect<i32, Physical>> = Vec::new();
    while let Some(mut rect) = rects.pop() {
        loop {
            let before = rects.len();
            rects.retain(|other| match rect.overlaps(*other) {
                true => {
                    rect = rect.merge(*other);
                    false
                }
                false => true,
            });
            if rects.len() == before {
                break;
            }
        }
        merged.push(rect);
    }
    match merged.len() > AT_MOST {
        true => rects.extend(merged.into_iter().reduce(|all, rect| all.merge(rect))),
        false => rects.append(&mut merged),
    }
}

#[derive(PartialEq)]
struct Filter {
    radius: u32,
    lens: Option<Lens>,
    cut_to: Option<Rect<i32, Physical>>,
}

fn filter_of(current: &PaneState) -> Option<Filter> {
    let look = current.look?;
    Some(Filter {
        radius: look.radius.to_bits(),
        lens: look.lens,
        cut_to: look
            .lens
            .filter(|lens| lens.refracts())
            .map(|_| current.geometry),
    })
}
