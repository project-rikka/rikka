use std::borrow::Cow;
use std::ops::Range;

use animate_body::{Body, HumanPose};

use crate::intrusion::{Inside, PATH_FRAMES, intrusions, scan};
use crate::mover::{MOST, Mover, STILL};
use crate::parallel::across;
use crate::physique::Physique;
use crate::solve::{Window, solve};

const BLOCK: usize = 64;
const OVERLAP: usize = 16;
const CONTEXT: usize = 2 * PATH_FRAMES;
const SCANNED_TOGETHER: usize = 32;

type Turns = [Vec<[f32; MOST]>; Mover::ALL.len()];

struct Solved {
    block: usize,
    turns: Turns,
}

impl Physique {
    pub const REACH: u32 = (BLOCK + 2 * OVERLAP + CONTEXT) as u32;

    #[must_use]
    pub fn fit(
        &self,
        body: &Body,
        first: u32,
        unfitted: &[(HumanPose, [bool; 2])],
        desired: Range<usize>,
    ) -> Vec<HumanPose> {
        let first = first as usize;
        if desired.is_empty() {
            return Vec::new();
        }
        let start = first + desired.start;
        let end = first + desired.end;
        let blocks: Vec<usize> =
            (start.saturating_sub(OVERLAP) / BLOCK..=(end - 1 + OVERLAP) / BLOCK).collect();
        let inside: Vec<Inside> = across(unfitted, SCANNED_TOGETHER, |(pose, _)| {
            scan(self, body, pose)
        });
        let solved = across(&blocks, 1, |block| {
            self.window(body, first, unfitted, &inside, *block)
        });
        desired
            .map(|at| {
                let frame = first + at;
                let mut pose = unfitted[at].0;
                for mover in Mover::ALL {
                    let turns = blended(&solved, mover, frame)
                        .map(|value| value.signum() * (value.abs() - STILL).max(0.0));
                    mover.apply(body, &mut pose, &turns);
                }
                pose
            })
            .collect()
    }

    fn window(
        &self,
        body: &Body,
        first: usize,
        unfitted: &[(HumanPose, [bool; 2])],
        inside: &[Inside],
        block: usize,
    ) -> Solved {
        let span = extent(block);
        let within = |frame: usize| frame.clamp(first, first + unfitted.len()) - first;
        let solving = within(span.start)..within(span.end);
        let context = within(span.start.saturating_sub(CONTEXT))..within(span.end + CONTEXT);
        let offset = span.start.max(first) - span.start;
        let mut turns: Turns = std::array::from_fn(|_| vec![[0.0; MOST]; offset + solving.len()]);
        if inside[solving.clone()].iter().all(Vec::is_empty) {
            return Solved { block, turns };
        }
        let mut poses: Vec<HumanPose> = unfitted[context.clone()]
            .iter()
            .map(|(pose, _)| *pose)
            .collect();
        let mut inside = Cow::Borrowed(&inside[context.clone()]);
        let local = solving.start - context.start..solving.end - context.start;
        let mut detected = intrusions(self, body, &poses, &inside);
        for mover in Mover::ALL {
            let pinned: Vec<bool> = unfitted[solving.clone()]
                .iter()
                .map(|(_, pinned)| match mover {
                    Mover::Arm(side) => pinned[side as usize],
                    Mover::Legs => false,
                })
                .collect();
            let solved = solve(&Window {
                physique: self,
                body,
                mover,
                poses: &poses[local.clone()],
                known: &detected[local.clone()],
                pinned: &pinned,
            });
            if mover == Mover::Legs && solved.iter().flatten().any(|value| *value != 0.0) {
                for (at, x) in local.clone().zip(&solved) {
                    mover.apply(body, &mut poses[at], x);
                    inside.to_mut()[at] = scan(self, body, &poses[at]);
                }
                detected = intrusions(self, body, &poses, &inside);
            }
            turns[mover.index()][offset..].copy_from_slice(&solved);
        }
        Solved { block, turns }
    }
}

fn extent(block: usize) -> Range<usize> {
    (block * BLOCK).saturating_sub(OVERLAP)..(block + 1) * BLOCK + OVERLAP
}

fn blended(solved: &[Solved], mover: Mover, frame: usize) -> [f32; MOST] {
    let mut sum = [0.0f32; MOST];
    let mut total = 0.0;
    let first = solved.first().map_or(0, |solution| solution.block);
    let near = (frame.saturating_sub(OVERLAP) / BLOCK).saturating_sub(first)
        ..=((frame + OVERLAP) / BLOCK).saturating_sub(first);
    for solution in near.filter_map(|at| solved.get(at)) {
        let span = extent(solution.block);
        if !span.contains(&frame) {
            continue;
        }
        let Some(turns) = solution.turns[mover.index()].get(frame - span.start) else {
            continue;
        };
        let weight = share(solution.block, frame);
        total += weight;
        for (sum, value) in sum.iter_mut().zip(turns) {
            *sum += weight * value;
        }
    }
    match total > 0.0 {
        true => sum.map(|value| value / total),
        false => sum,
    }
}

fn share(block: usize, frame: usize) -> f32 {
    let rising =
        (frame as f32 + 0.5 - (block * BLOCK) as f32 + OVERLAP as f32) / (2 * OVERLAP) as f32;
    let falling =
        (((block + 1) * BLOCK + OVERLAP) as f32 - frame as f32 - 0.5) / (2 * OVERLAP) as f32;
    smooth(rising) * smooth(falling)
}

fn smooth(along: f32) -> f32 {
    let along = along.clamp(0.0, 1.0);
    along * along * (3.0 - 2.0 * along)
}
