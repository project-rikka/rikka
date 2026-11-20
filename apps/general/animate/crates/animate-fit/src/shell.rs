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

enum Level {
    Within(f32),
    Beyond(f32),
}

impl Shell {
    pub(crate) fn closed(axis: Vec3, slice: f32, points: &[Vec3]) -> Option<Self> {
        let (mut shell, sampled) =
            Self::gathered(axis, slice, points.iter().map(|point| (*point, 0.0)))?;
        for ring in &mut shell.rings {
            fill(ring);
        }
        fill_between(&mut shell.rings, &sampled);
        Some(shell)
    }

    pub(crate) fn open(axis: Vec3, slice: f32, insets: &[(Vec3, f32)]) -> Option<Self> {
        let (mut shell, sampled) = Self::gathered(axis, slice, insets.iter().copied())?;
        fill_between(&mut shell.rings, &sampled);
        Some(shell)
    }

    fn gathered(
        axis: Vec3,
        slice: f32,
        insets: impl Iterator<Item = (Vec3, f32)> + Clone,
    ) -> Option<(Self, Vec<bool>)> {
        let first = axis.any_orthonormal_vector();
        let mut shell = Self {
            axis,
            across: [first, axis.cross(first)],
            from: 0.0,
            slice,
            rings: Vec::new(),
        };
        let (from, to) = insets
            .clone()
            .map(|(point, _)| point.dot(axis))
            .fold((f32::MAX, f32::MIN), |(low, high), height| {
                (low.min(height), high.max(height))
            });
        if from > to {
            return None;
        }
        shell.from = from;
        let count = ((to - from) / slice) as usize + 1;
        let mut rings = vec![[0.0f32; SECTORS]; count];
        let mut sampled = vec![false; count];
        for (point, inset) in insets {
            let spot = shell.spot(point);
            let level = ((spot.height - from) / slice) as usize;
            let sector = (spot.sector as usize).min(SECTORS - 1);
            rings[level][sector] = rings[level][sector].max(spot.radius - inset);
            sampled[level] = true;
        }
        shell.rings = rings;
        Some((shell, sampled))
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
        match self.level(&spot) {
            Level::Beyond(beyond) => beyond * self.slice + spot.radius,
            Level::Within(level) => spot.radius - self.edge_at(level, spot.sector),
        }
    }

    pub(crate) fn edge(&self, local: Vec3) -> Option<f32> {
        let spot = self.spot(local);
        match self.level(&spot) {
            Level::Beyond(_) => None,
            Level::Within(level) => Some(self.edge_at(level, spot.sector)),
        }
    }

    fn level(&self, spot: &Spot) -> Level {
        let level = (spot.height - self.from) / self.slice - 0.5;
        let last = self.rings.len() as f32 - 1.0;
        let beyond = (-0.5 - level).max(level - last - 0.5);
        match beyond > 0.0 {
            true => Level::Beyond(beyond),
            false => Level::Within(level.clamp(0.0, last)),
        }
    }

    fn edge_at(&self, level: f32, sector: f32) -> f32 {
        let below = level.floor() as usize;
        let above = (below + 1).min(self.rings.len() - 1);
        let rise = level - below as f32;
        let around = |ring: &[f32; SECTORS]| {
            let centred = sector - 0.5;
            let low = centred.floor();
            let share = centred - low;
            let sector = (low as isize).rem_euclid(SECTORS as isize) as usize;
            ring[sector] + (ring[(sector + 1) % SECTORS] - ring[sector]) * share
        };
        around(&self.rings[below]) * (1.0 - rise) + around(&self.rings[above]) * rise
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

fn fill_between(rings: &mut [[f32; SECTORS]], sampled: &[bool]) {
    let filled: Vec<usize> = (0..rings.len()).filter(|ring| sampled[*ring]).collect();
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
