use rikka_types::{Point, Rect};

use crate::area::GAP;

pub const REACH: f32 = 8.0;

const EXACT: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    Vertical { x: f32, top: f32, bottom: f32 },
    Horizontal { y: f32, left: f32, right: f32 },
    Spacing { span: Rect, across_x: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Snapped {
    pub rect: Rect,
    pub guides: Vec<Guide>,
}

#[derive(Clone, Copy, Debug)]
pub struct Snapper<'a> {
    room: Rect,
    others: &'a [Rect],
    reach: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Run {
    start: f32,
    end: f32,
}

#[derive(Clone, Debug)]
struct Candidate {
    delta: f32,
    rank: u8,
    guides: Vec<Guide>,
}

impl<'a> Snapper<'a> {
    #[must_use]
    pub fn new(room: Rect, others: &'a [Rect]) -> Snapper<'a> {
        Snapper {
            room,
            others,
            reach: REACH,
        }
    }

    #[must_use]
    pub fn reach(self, reach: f32) -> Snapper<'a> {
        Snapper {
            reach: reach.max(0.0),
            ..self
        }
    }

    #[must_use]
    pub fn snap(&self, moving: Rect) -> Snapped {
        let dx = self.nearest(Axis::X, moving);
        let dy = self.nearest(Axis::Y, moving);
        let rect = moving.translate(Point::new(dx, dy));
        Snapped {
            rect,
            guides: self.matches(rect),
        }
    }

    #[must_use]
    pub fn matches(&self, rect: Rect) -> Vec<Guide> {
        let mut guides: Vec<Guide> = [Axis::X, Axis::Y]
            .into_iter()
            .flat_map(|axis| self.candidates(axis, rect))
            .filter(|candidate| candidate.delta.abs() < EXACT)
            .flat_map(|candidate| candidate.guides)
            .collect();
        merge(&mut guides);
        guides
    }

    fn nearest(&self, axis: Axis, moving: Rect) -> f32 {
        self.candidates(axis, moving)
            .into_iter()
            .filter(|candidate| candidate.delta.abs() <= self.reach)
            .min_by(|a, b| {
                a.delta
                    .abs()
                    .total_cmp(&b.delta.abs())
                    .then(a.rank.cmp(&b.rank))
            })
            .map_or(0.0, |candidate| candidate.delta)
    }

    fn candidates(&self, axis: Axis, moving: Rect) -> Vec<Candidate> {
        let mut candidates = Vec::new();
        self.room_candidates(axis, moving, &mut candidates);
        for other in self.others {
            aligned_with(axis, moving, *other, &mut candidates);
        }
        self.gap_candidates(axis, moving, &mut candidates);
        candidates
    }

    fn room_candidates(&self, axis: Axis, moving: Rect, candidates: &mut Vec<Candidate>) {
        let (room, own) = (run(self.room, axis), run(moving, axis));
        let across = run(self.room, axis.other());
        for (target, anchor) in [
            (room.start, own.start),
            (room.center(), own.center()),
            (room.end, own.end),
        ] {
            candidates.push(Candidate {
                delta: target - anchor,
                rank: 1,
                guides: vec![line(axis, target, across)],
            });
        }
    }

    fn gap_candidates(&self, axis: Axis, moving: Rect, candidates: &mut Vec<Candidate>) {
        let own = run(moving, axis);
        let before = self.neighbour(axis, moving, Side::Before);
        let after = self.neighbour(axis, moving, Side::After);

        if let Some(left) = before {
            let at = run(left, axis).end + GAP;
            candidates.push(Candidate {
                delta: at - own.start,
                rank: 0,
                guides: vec![spacing(axis, left, moving, run(left, axis).end, own.start)],
            });
            if let Some(further) = self.neighbour(axis, left, Side::Before) {
                let gap = run(left, axis).start - run(further, axis).end;
                if gap > 0.0 {
                    candidates.push(Candidate {
                        delta: run(left, axis).end + gap - own.start,
                        rank: 0,
                        guides: vec![
                            spacing(
                                axis,
                                further,
                                left,
                                run(further, axis).end,
                                run(left, axis).start,
                            ),
                            spacing(axis, left, moving, run(left, axis).end, own.start),
                        ],
                    });
                }
            }
        }
        if let Some(right) = after {
            let at = run(right, axis).start - GAP;
            candidates.push(Candidate {
                delta: at - own.end,
                rank: 0,
                guides: vec![spacing(
                    axis,
                    moving,
                    right,
                    own.end,
                    run(right, axis).start,
                )],
            });
            if let Some(further) = self.neighbour(axis, right, Side::After) {
                let gap = run(further, axis).start - run(right, axis).end;
                if gap > 0.0 {
                    candidates.push(Candidate {
                        delta: run(right, axis).start - gap - own.end,
                        rank: 0,
                        guides: vec![
                            spacing(axis, moving, right, own.end, run(right, axis).start),
                            spacing(
                                axis,
                                right,
                                further,
                                run(right, axis).end,
                                run(further, axis).start,
                            ),
                        ],
                    });
                }
            }
        }
        if let (Some(left), Some(right)) = (before, after) {
            let free = run(right, axis).start - run(left, axis).end - own.length();
            if free >= 0.0 {
                let start = run(left, axis).end + free / 2.0;
                let end = start + own.length();
                candidates.push(Candidate {
                    delta: start - own.start,
                    rank: 0,
                    guides: vec![
                        spacing(axis, left, moving, run(left, axis).end, start),
                        spacing(axis, moving, right, end, run(right, axis).start),
                    ],
                });
            }
        }
    }

    fn neighbour(&self, axis: Axis, of: Rect, side: Side) -> Option<Rect> {
        let own = run(of, axis);
        let across = run(of, axis.other());
        self.others
            .iter()
            .copied()
            .filter(|other| *other != of && run(*other, axis.other()).overlaps(across))
            .filter(|other| match side {
                Side::Before => run(*other, axis).end <= own.center(),
                Side::After => run(*other, axis).start >= own.center(),
            })
            .min_by(|a, b| {
                let distance = |rect: &Rect| match side {
                    Side::Before => own.start - run(*rect, axis).end,
                    Side::After => run(*rect, axis).start - own.end,
                };
                distance(a).total_cmp(&distance(b))
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Before,
    After,
}

fn aligned_with(axis: Axis, moving: Rect, other: Rect, candidates: &mut Vec<Candidate>) {
    let (own, theirs) = (run(moving, axis), run(other, axis));
    let across = run(moving, axis.other()).union(run(other, axis.other()));
    for anchor in [own.start, own.center(), own.end] {
        for target in [theirs.start, theirs.center(), theirs.end] {
            candidates.push(Candidate {
                delta: target - anchor,
                rank: 0,
                guides: vec![line(axis, target, across)],
            });
        }
    }
}

fn line(axis: Axis, at: f32, across: Run) -> Guide {
    match axis {
        Axis::X => Guide::Vertical {
            x: at,
            top: across.start,
            bottom: across.end,
        },
        Axis::Y => Guide::Horizontal {
            y: at,
            left: across.start,
            right: across.end,
        },
    }
}

fn spacing(axis: Axis, first: Rect, second: Rect, from: f32, to: f32) -> Guide {
    let (one, two) = (run(first, axis.other()), run(second, axis.other()));
    let across = one.intersection(two).unwrap_or(two);
    let span = match axis {
        Axis::X => Rect::new(from, across.start, to - from, across.length()),
        Axis::Y => Rect::new(across.start, from, across.length(), to - from),
    };
    Guide::Spacing {
        span,
        across_x: axis == Axis::X,
    }
}

fn merge(guides: &mut Vec<Guide>) {
    let mut merged: Vec<Guide> = Vec::with_capacity(guides.len());
    for guide in guides.drain(..) {
        let joined = merged
            .iter_mut()
            .any(|merged_guide| join(merged_guide, guide));
        if !joined {
            merged.push(guide);
        }
    }
    *guides = merged;
}

fn join(merged_guide: &mut Guide, guide: Guide) -> bool {
    match (merged_guide, guide) {
        (
            Guide::Vertical { x, top, bottom },
            Guide::Vertical {
                x: at,
                top: from,
                bottom: to,
            },
        ) if (*x - at).abs() < EXACT => {
            *top = top.min(from);
            *bottom = bottom.max(to);
            true
        }
        (
            Guide::Horizontal { y, left, right },
            Guide::Horizontal {
                y: at,
                left: from,
                right: to,
            },
        ) if (*y - at).abs() < EXACT => {
            *left = left.min(from);
            *right = right.max(to);
            true
        }
        (merged_guide @ Guide::Spacing { .. }, guide @ Guide::Spacing { .. }) => {
            *merged_guide == guide
        }
        _ => false,
    }
}

fn run(rect: Rect, axis: Axis) -> Run {
    match axis {
        Axis::X => Run {
            start: rect.min_x(),
            end: rect.max_x(),
        },
        Axis::Y => Run {
            start: rect.min_y(),
            end: rect.max_y(),
        },
    }
}

impl Axis {
    fn other(self) -> Axis {
        match self {
            Axis::X => Axis::Y,
            Axis::Y => Axis::X,
        }
    }
}

impl Run {
    fn center(self) -> f32 {
        (self.start + self.end) / 2.0
    }

    fn length(self) -> f32 {
        self.end - self.start
    }

    fn overlaps(self, other: Run) -> bool {
        self.start < other.end && other.start < self.end
    }

    fn union(self, other: Run) -> Run {
        Run {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    fn intersection(self, other: Run) -> Option<Run> {
        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        (start < end).then_some(Run { start, end })
    }
}
