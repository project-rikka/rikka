use animate_body::{Body, HumanPose};
use rikka_model_core::glam::Vec3;

use crate::band::Band;
use crate::intrusion::Intrusion;
use crate::mover::{MOST, Mover, Prepared, STILL, limited};
use crate::physique::Physique;
use crate::scene::Scene;

const ITERATIONS: usize = 16;
const KEEP: f64 = 0.0015;
const VELOCITY: f64 = 0.05;
const ACCELERATION: f64 = 2.0;
const WRIST: f64 = 0.01;
const NUDGE: f32 = 1e-3;
const HELD_WRIST: f64 = 20.0;
const BACKTRACKS: usize = 6;
const CONVERGED: f64 = 1e-4;

#[derive(Clone, Copy)]
enum Row {
    Known(Intrusion),
    Fresh(usize, usize),
}

impl Row {
    fn depth(&self, scene: &Scene<'_>) -> f32 {
        match self {
            Self::Known(intrusion) => {
                let local = scene.local(intrusion.probe, intrusion.obstacle);
                let radial = scene.depth_at(intrusion.probe, intrusion.obstacle, local);
                match radial > 0.0 {
                    true => intrusion.depth(local).max(radial),
                    false => radial,
                }
            }
            Self::Fresh(probe, obstacle) => scene.depth(*probe, *obstacle),
        }
    }
}

pub(crate) struct Window<'a> {
    pub(crate) physique: &'a Physique,
    pub(crate) body: &'a Body,
    pub(crate) mover: Mover,
    pub(crate) poses: &'a [HumanPose],
    pub(crate) known: &'a [Vec<Intrusion>],
    pub(crate) pinned: &'a [bool],
}

struct Look {
    rows: Vec<(Row, f32)>,
    wrist: Vec3,
}

struct Solver<'a> {
    window: &'a Window<'a>,
    prepared: Vec<Prepared>,
    detected: Vec<Vec<Intrusion>>,
    dims: usize,
    costs: [f64; MOST],
}

pub(crate) fn solve(window: &Window<'_>) -> Vec<[f32; MOST]> {
    let frames = window.poses.len();
    let still = vec![[0.0; MOST]; frames];
    let mover = window.mover;
    let moved = |probe: usize| mover.moves(window.physique.probes[probe].region);
    let detected: Vec<Vec<Intrusion>> = window
        .known
        .iter()
        .map(|frame_intrusions| {
            frame_intrusions
                .iter()
                .filter(|intrusion| moved(intrusion.probe))
                .copied()
                .collect()
        })
        .collect();
    if detected.iter().all(Vec::is_empty) {
        return still;
    }
    let solver = Solver {
        window,
        prepared: window
            .poses
            .iter()
            .map(|pose| mover.prepare(window.body, pose))
            .collect(),
        detected,
        dims: mover.dims(),
        costs: mover.costs().map(f64::from),
    };
    solver.run(still)
}

impl Solver<'_> {
    fn run(&self, mut x: Vec<[f32; MOST]>) -> Vec<[f32; MOST]> {
        let mut looks = self.looks(&x);
        let mut energy = self.energy(&x, &looks);
        for _ in 0..ITERATIONS {
            let Some(change) = self.step(&x, &looks) else {
                break;
            };
            let mut share = 1.0;
            let mut accepted = None;
            for _ in 0..BACKTRACKS {
                let tried: Vec<[f32; MOST]> = x
                    .iter()
                    .zip(&change)
                    .map(|(intrusion, change)| {
                        let moved = std::array::from_fn(|dim| intrusion[dim] + change[dim] * share);
                        limited(moved)
                    })
                    .collect();
                let tried_looks = self.looks(&tried);
                let tried_energy = self.energy(&tried, &tried_looks);
                if tried_energy < energy {
                    accepted = Some((tried, tried_looks, tried_energy));
                    break;
                }
                share *= 0.5;
            }
            let Some((tried, tried_looks, tried_energy)) = accepted else {
                break;
            };
            let gained = (energy - tried_energy) / energy.max(f64::MIN_POSITIVE);
            (x, looks, energy) = (tried, tried_looks, tried_energy);
            if gained < CONVERGED {
                break;
            }
        }
        x
    }

    fn live(&self, frame: usize, x: &[f32; MOST]) -> bool {
        !self.detected[frame].is_empty() || x.iter().any(|value| value.abs() > STILL)
    }

    fn looks(&self, x: &[[f32; MOST]]) -> Vec<Option<Look>> {
        x.iter()
            .enumerate()
            .map(|(frame, x)| self.live(frame, x).then(|| self.look(frame, x)))
            .collect()
    }

    fn look(&self, frame: usize, x: &[f32; MOST]) -> Look {
        let window = self.window;
        let prepared = &self.prepared[frame];
        let scene = prepared.scene(window.physique, window.body, x);
        let intrusions = &self.detected[frame];
        let mut rows: Vec<(Row, f32)> = intrusions
            .iter()
            .map(|intrusion| Row::Known(*intrusion))
            .map(|row| (row, row.depth(&scene)))
            .filter(|(_, depth)| *depth > 0.0)
            .collect();
        for ((probe, obstacle), depth) in scene.inside(|region| window.mover.moves(region)) {
            let pairs = intrusions
                .iter()
                .any(|intrusion| intrusion.probe == probe && intrusion.obstacle == obstacle);
            if !pairs {
                rows.push((Row::Fresh(probe, obstacle), depth));
            }
        }
        Look {
            wrist: prepared.wrist(&scene),
            rows,
        }
    }

    fn wrist_weight(&self, frame: usize) -> f64 {
        match self.window.pinned[frame] {
            true => HELD_WRIST,
            false => WRIST,
        }
    }

    fn energy(&self, x: &[[f32; MOST]], looks: &[Option<Look>]) -> f64 {
        let scale = f64::from(self.window.physique.scale);
        let mut energy = 0.0;
        for (frame, look) in looks.iter().enumerate() {
            if let Some(look) = look {
                let rows: f64 = look
                    .rows
                    .iter()
                    .map(|(_, depth)| (f64::from(*depth) / scale).powi(2))
                    .sum();
                let wrist = f64::from(look.wrist.length_squared()) / (scale * scale);
                energy += rows + self.wrist_weight(frame) * wrist;
            }
        }
        for stencil in stencils(x.len()) {
            for (dim, cost) in self.costs.iter().enumerate().take(self.dims) {
                let value = stencil.value(x, dim);
                energy += stencil.weight * cost * value * value;
            }
        }
        energy
    }

    fn step(&self, x: &[[f32; MOST]], looks: &[Option<Look>]) -> Option<Vec<[f32; MOST]>> {
        let window = self.window;
        let dims = self.dims;
        let frames = x.len();
        let scale = f64::from(window.physique.scale);
        let mut band = Band::new(frames * dims, 2 * dims);
        let mut right = vec![0.0f64; frames * dims];
        for (frame, look) in looks.iter().enumerate() {
            let Some(look) = look else {
                continue;
            };
            let prepared = &self.prepared[frame];
            let wrist_weight = self.wrist_weight(frame);
            let mut slopes = vec![[0.0f64; MOST]; look.rows.len()];
            let mut wrist_slopes = [[0.0f64; MOST]; 3];
            for dim in 0..dims {
                let mut nudged = x[frame];
                nudged[dim] += NUDGE;
                let scene = prepared.scene(window.physique, window.body, &nudged);
                for (slope, (row, depth)) in slopes.iter_mut().zip(&look.rows) {
                    slope[dim] = f64::from(row.depth(&scene) - depth) / f64::from(NUDGE) / scale;
                }
                let wrist = prepared.wrist(&scene) - look.wrist;
                for (axis, slope) in wrist_slopes.iter_mut().enumerate() {
                    slope[dim] = f64::from(wrist[axis]) / f64::from(NUDGE) / scale;
                }
            }
            let residuals = look
                .rows
                .iter()
                .map(|(_, depth)| (f64::from(*depth) / scale, 1.0))
                .zip(&slopes)
                .chain(
                    look.wrist
                        .to_array()
                        .into_iter()
                        .map(|value| (f64::from(value) / scale, wrist_weight))
                        .zip(&wrist_slopes),
                );
            for ((residual, weight), slope) in residuals {
                for one in 0..dims {
                    right[frame * dims + one] -= weight * slope[one] * residual;
                    for two in 0..=one {
                        band.add(
                            frame * dims + one,
                            frame * dims + two,
                            weight * slope[one] * slope[two],
                        );
                    }
                }
            }
        }
        for stencil in stencils(frames) {
            for dim in 0..dims {
                let weight = stencil.weight * self.costs[dim];
                let value = stencil.value(x, dim);
                for (one, factor) in stencil.terms() {
                    right[one * dims + dim] -= weight * factor * value;
                    for (two, other) in stencil.terms().take_while(|(two, _)| *two <= one) {
                        band.add(one * dims + dim, two * dims + dim, weight * factor * other);
                    }
                }
            }
        }
        let solved = band.solve(right)?;
        Some(
            (0..frames)
                .map(|frame| {
                    std::array::from_fn(|dim| match dim < dims {
                        true => solved[frame * dims + dim] as f32,
                        false => 0.0,
                    })
                })
                .collect(),
        )
    }
}

const KEPT: [f64; 1] = [1.0];
const SPEED: [f64; 2] = [-1.0, 1.0];
const BEND: [f64; 3] = [1.0, -2.0, 1.0];

struct Stencil {
    weight: f64,
    first: usize,
    factors: &'static [f64],
}

impl Stencil {
    fn terms(&self) -> impl Iterator<Item = (usize, f64)> + '_ {
        self.factors
            .iter()
            .enumerate()
            .map(|(at, factor)| (self.first + at, *factor))
    }

    fn value(&self, x: &[[f32; MOST]], dim: usize) -> f64 {
        self.terms()
            .map(|(frame, factor)| factor * f64::from(x[frame][dim]))
            .sum()
    }
}

fn stencils(frames: usize) -> impl Iterator<Item = Stencil> {
    [
        (KEEP, &KEPT[..]),
        (VELOCITY, &SPEED[..]),
        (ACCELERATION, &BEND[..]),
    ]
    .into_iter()
    .flat_map(move |(weight, factors)| {
        (0..(frames + 1).saturating_sub(factors.len())).map(move |first| Stencil {
            weight,
            first,
            factors,
        })
    })
}
