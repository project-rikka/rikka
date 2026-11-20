use rikka_model_core::HumanBone;
use rikka_model_core::glam::Vec3;

use crate::limb::Side;
use crate::worlds::Worlds;

const FLAT: f32 = 0.002;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sole {
    pub heel: Vec3,
    pub toe: Vec3,
}

fn lowest(points: &[Vec3], forward: bool) -> Option<Vec3> {
    let floor = points.iter().map(|point| point.y).min_by(f32::total_cmp)?;
    points
        .iter()
        .copied()
        .filter(|point| point.y <= floor + FLAT)
        .max_by(|one, other| match forward {
            true => one.z.total_cmp(&other.z),
            false => other.z.total_cmp(&one.z),
        })
}

impl Sole {
    pub(crate) fn measure(side: Side, rest: &Worlds, flesh: &[(HumanBone, Vec3)]) -> Self {
        let (foot, toes) = (side.foot(), side.toes());
        let ankle = rest.joint(foot);
        let tip = rest.get(toes).unwrap_or(ankle);
        let points: Vec<Vec3> = flesh
            .iter()
            .filter(|(bone, _)| *bone == foot || *bone == toes)
            .map(|(_, point)| *point)
            .collect();
        let reach = points
            .iter()
            .map(|point| point.z)
            .fold((f32::MAX, f32::MIN), |(low, high), z| {
                (low.min(z), high.max(z))
            });
        let middle = (reach.0 + reach.1) * 0.5;
        let back: Vec<Vec3> = points
            .iter()
            .copied()
            .filter(|point| point.z < middle)
            .collect();
        let front: Vec<Vec3> = points
            .iter()
            .copied()
            .filter(|point| point.z >= middle)
            .collect();
        let grounded = |point: Vec3| Vec3::new(point.x, 0.0, point.z);
        let heel = lowest(&back, false).unwrap_or_else(|| grounded(ankle.position));
        let toe = lowest(&front, true).unwrap_or_else(|| grounded(tip.position));
        Self {
            heel: ankle.local(heel),
            toe: tip.local(toe),
        }
    }
}
