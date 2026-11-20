use std::f32::consts::TAU;

use rikka_model_core::glam::Vec3;

const SECTORS: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Shell {
    axis: Vec3,
    across: [Vec3; 2],
    from: f32,
    slice: f32,
    rings: Vec<[f32; SECTORS]>,
}

struct Spot {
    height: f32,
    sector: f32,
    radius: f32,
}

impl Shell {
    pub(crate) fn fit(axis: Vec3, slice: f32, points: &[Vec3]) -> Option<Self> {
        let first = axis.any_orthonormal_vector();
        let mut shell = Self {
            axis,
            across: [first, axis.cross(first)],
            from: 0.0,
            slice,
            rings: Vec::new(),
        };
        let spots: Vec<Spot> = points.iter().map(|point| shell.spot(*point)).collect();
        let (from, to) = spots
            .iter()
            .fold((f32::MAX, f32::MIN), |(low, high), spot| {
                (low.min(spot.height), high.max(spot.height))
            });
        if from > to {
            return None;
        }
        shell.from = from;
        let mut rings = vec![[0.0f32; SECTORS]; ((to - from) / slice) as usize + 1];
        for spot in &spots {
            let ring = &mut rings[((spot.height - from) / slice) as usize];
            let sector = (spot.sector as usize).min(SECTORS - 1);
            ring[sector] = ring[sector].max(spot.radius);
        }
        for ring in &mut rings {
            fill(ring);
        }
        fill_between(&mut rings);
        shell.rings = rings;
        Some(shell)
    }

    fn spot(&self, local: Vec3) -> Spot {
        let (x, y) = (local.dot(self.across[0]), local.dot(self.across[1]));
        Spot {
            height: local.dot(self.axis),
            sector: (y.atan2(x) / TAU).rem_euclid(1.0) * SECTORS as f32,
            radius: x.hypot(y),
        }
    }

    pub(crate) fn bound(&self) -> (Vec3, f32) {
        let length = self.rings.len() as f32 * self.slice;
        let widest = self
            .rings
            .iter()
            .flat_map(|ring| ring.iter())
            .fold(0.0f32, |widest, radius| widest.max(*radius));
        let centre = self.axis * (self.from + length * 0.5);
        (centre, (length * 0.5).hypot(widest))
    }

    pub(crate) fn distance(&self, local: Vec3) -> f32 {
        let spot = self.spot(local);
        let level = (spot.height - self.from) / self.slice - 0.5;
        let last = self.rings.len() as f32 - 1.0;
        let beyond = (-0.5 - level).max(level - last - 0.5);
        if beyond > 0.0 {
            return beyond * self.slice + spot.radius.max(0.0);
        }
        let level = level.clamp(0.0, last);
        let below = level.floor() as usize;
        let above = (below + 1).min(self.rings.len() - 1);
        let rise = level - below as f32;
        let edge = |ring: &[f32; SECTORS]| {
            let centred = spot.sector - 0.5;
            let low = centred.floor();
            let share = centred - low;
            let sector = (low as isize).rem_euclid(SECTORS as isize) as usize;
            ring[sector] + (ring[(sector + 1) % SECTORS] - ring[sector]) * share
        };
        let edge = edge(&self.rings[below]) * (1.0 - rise) + edge(&self.rings[above]) * rise;
        spot.radius - edge
    }
}

fn fill(ring: &mut [f32; SECTORS]) {
    let filled: Vec<usize> = (0..SECTORS).filter(|sector| ring[*sector] > 0.0).collect();
    if filled.is_empty() || filled.len() == SECTORS {
        return;
    }
    let original = *ring;
    for sector in 0..SECTORS {
        if original[sector] > 0.0 {
            continue;
        }
        let after = filled
            .iter()
            .copied()
            .find(|other| *other > sector)
            .unwrap_or(filled[0] + SECTORS);
        let before = filled
            .iter()
            .rev()
            .copied()
            .find(|other| *other < sector)
            .map_or(
                filled[filled.len() - 1] as isize - SECTORS as isize,
                |other| other as isize,
            );
        let span = (after as isize - before) as f32;
        let share = (sector as isize - before) as f32 / span;
        let one = original[before.rem_euclid(SECTORS as isize) as usize];
        let two = original[after % SECTORS];
        ring[sector] = one + (two - one) * share;
    }
}

fn fill_between(rings: &mut [[f32; SECTORS]]) {
    let filled: Vec<usize> = (0..rings.len())
        .filter(|ring| rings[*ring].iter().any(|radius| *radius > 0.0))
        .collect();
    for ring in 0..rings.len() {
        if filled.contains(&ring) {
            continue;
        }
        let before = filled.iter().rev().copied().find(|other| *other < ring);
        let after = filled.iter().copied().find(|other| *other > ring);
        rings[ring] = match (before, after) {
            (Some(one), Some(two)) => {
                let share = (ring - one) as f32 / (two - one) as f32;
                let mut mixed = rings[one];
                for (slot, other) in mixed.iter_mut().zip(rings[two]) {
                    *slot += (other - *slot) * share;
                }
                mixed
            }
            (Some(one), None) => rings[one],
            (None, Some(two)) => rings[two],
            (None, None) => [0.0; SECTORS],
        };
    }
}
