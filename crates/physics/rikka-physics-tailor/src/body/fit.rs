use glam::Vec3;

use super::chain::chains;
use super::hull::Hull;
use super::solid::Solid;
use crate::geometry::Principal;

const LOBES: usize = 3;
const SCORED: usize = 384;
const PER_SOLID: f32 = 0.0005;
const FEWEST: usize = 8;
const HULL_WEIGHT: f32 = 4.0;

pub(crate) fn solids(points: &[Vec3]) -> Vec<Solid> {
    let Some(frame) = Principal::compute(points) else {
        return Vec::new();
    };
    let outline = Hull::fit(points, &frame);
    let skin = Skin::measure(points, &outline);
    let mut candidates: Vec<(f32, Vec<Solid>)> = Vec::new();
    let mut offer = |solids: Vec<Solid>| {
        if solids.is_empty() {
            return;
        }
        let weight: f32 = solids
            .iter()
            .map(|solid| match solid {
                Solid::Cone(_) => 1.0,
                Solid::Hull(_) => HULL_WEIGHT,
            })
            .sum();
        candidates.push((skin.excess(&solids) + PER_SOLID * weight, solids));
    };
    for along in 0..3 {
        let across = match along {
            0 => [1, 2],
            1 => [0, 2],
            _ => [0, 1],
        };
        let across = if frame.half[across[0]] >= frame.half[across[1]] {
            across[0]
        } else {
            across[1]
        };
        for lobes in 1..=LOBES {
            offer(lobed(points, &frame, &outline, along, across, lobes));
        }
    }
    offer(vec![Solid::Hull(outline.clone())]);
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    candidates
        .into_iter()
        .map(|(_, solids)| solids)
        .find(|solids| solids.iter().all(|solid| solid.is_solid()))
        .unwrap_or_default()
}

fn lobed(
    points: &[Vec3],
    frame: &Principal,
    outline: &Hull,
    along: usize,
    across: usize,
    lobes: usize,
) -> Vec<Solid> {
    let width = frame.half[across] * 2.0 / lobes as f32;
    if lobes > 1 && width <= f32::EPSILON {
        return Vec::new();
    }
    let mut groups: Vec<Vec<Vec3>> = vec![Vec::new(); lobes];
    for point in points {
        let place = (frame.local(*point)[across] + frame.half[across]) / width;
        groups[(place.max(0.0) as usize).min(lobes - 1)].push(*point);
    }
    for index in 1..groups.len() {
        if groups[index - 1].len() < FEWEST {
            let moved = std::mem::take(&mut groups[index - 1]);
            groups[index].extend(moved);
        }
    }
    if let [.., before, last] = groups.as_mut_slice()
        && last.len() < FEWEST
    {
        before.append(last);
    }
    let mut solids = Vec::new();
    for group in groups.iter().filter(|group| group.len() >= FEWEST) {
        let skin = Skin::measure(group, outline);
        let best = chains(group, frame.axes[along])
            .into_iter()
            .map(|cones| {
                let solids: Vec<Solid> = cones.into_iter().map(Solid::Cone).collect();
                (skin.excess(&solids), solids)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        solids.extend(best.into_iter().flat_map(|(_, solids)| solids));
    }
    solids
}

struct Skin {
    points: Vec<Vec3>,
    inner: Vec<f32>,
}

impl Skin {
    fn measure(points: &[Vec3], outline: &Hull) -> Self {
        let step = points.len().div_ceil(SCORED).max(1);
        let points: Vec<Vec3> = points.iter().step_by(step).copied().collect();
        let inner = points
            .iter()
            .map(|point| (-outline.distance(*point)).max(0.0))
            .collect();
        Self { points, inner }
    }

    fn excess(&self, solids: &[Solid]) -> f32 {
        if self.points.is_empty() {
            return 0.0;
        }
        let total: f32 = self
            .points
            .iter()
            .zip(&self.inner)
            .map(|(point, inner)| {
                let depth = solids
                    .iter()
                    .map(|solid| -solid.distance(*point))
                    .fold(0.0f32, f32::max);
                (depth - inner).max(0.0)
            })
            .sum();
        total / self.points.len() as f32
    }
}
