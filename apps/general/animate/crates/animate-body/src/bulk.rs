use std::f32::consts::TAU;

use rikka_model_core::HumanBone;
use rikka_model_core::glam::Vec3;

use crate::skeleton::Skeleton;
use crate::worlds::Worlds;

pub(crate) const TRUNK: [HumanBone; 10] = [
    HumanBone::Hips,
    HumanBone::Spine,
    HumanBone::Chest,
    HumanBone::UpperChest,
    HumanBone::Neck,
    HumanBone::Head,
    HumanBone::LeftUpperLeg,
    HumanBone::RightUpperLeg,
    HumanBone::LeftLowerLeg,
    HumanBone::RightLowerLeg,
];

const SECTORS: usize = 16;
const SLICE: f32 = 0.04;
const FARTHEST: f32 = 0.35;
const INTO_PARENT: f32 = 0.5;
const STRIDE: f32 = 0.01;
const LONGEST_WALK: usize = 200;
const EDGE_STEPS: usize = 10;

#[derive(Clone, Debug)]
pub struct Bulk {
    pieces: Vec<Piece>,
    stride: f32,
}

#[derive(Clone, Debug)]
struct Piece {
    bone: HumanBone,
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

impl Piece {
    fn measure(
        bone: HumanBone,
        axis: Vec3,
        lowest: f32,
        scale: f32,
        points: &[Vec3],
    ) -> Option<Self> {
        let first = axis.any_orthonormal_vector();
        let mut piece = Self {
            bone,
            axis,
            across: [first, axis.cross(first)],
            from: 0.0,
            slice: SLICE * scale,
            rings: Vec::new(),
        };
        let spots: Vec<Spot> = points
            .iter()
            .map(|point| piece.spot(*point))
            .filter(|spot| spot.height >= lowest && spot.radius <= FARTHEST * scale)
            .collect();
        let (from, to) = spots
            .iter()
            .fold((f32::MAX, f32::MIN), |(low, high), spot| {
                (low.min(spot.height), high.max(spot.height))
            });
        if from > to {
            return None;
        }
        piece.from = from;
        piece.rings = vec![[0.0; SECTORS]; ((to - from) / piece.slice) as usize + 1];
        for spot in spots {
            let ring = &mut piece.rings[((spot.height - from) / piece.slice) as usize];
            let sector = spot.sector as usize % SECTORS;
            ring[sector] = ring[sector].max(spot.radius);
        }
        Some(piece)
    }

    fn spot(&self, local: Vec3) -> Spot {
        let (x, y) = (local.dot(self.across[0]), local.dot(self.across[1]));
        Spot {
            height: local.dot(self.axis),
            sector: (y.atan2(x) / TAU).rem_euclid(1.0) * SECTORS as f32,
            radius: x.hypot(y),
        }
    }

    fn holds(&self, local: Vec3, gap: f32) -> bool {
        let spot = self.spot(local);
        let level = (spot.height - self.from) / self.slice;
        let reach = gap / self.slice;
        if level < -reach || level > self.rings.len() as f32 + reach {
            return false;
        }
        let ring = &self.rings[(level.max(0.0) as usize).min(self.rings.len() - 1)];
        let centred = spot.sector - 0.5;
        let below = centred.floor();
        let fraction = centred - below;
        let sector = (below as isize).rem_euclid(SECTORS as isize) as usize;
        let edge = ring[sector] + (ring[(sector + 1) % SECTORS] - ring[sector]) * fraction;
        edge > 0.0 && spot.radius < edge + gap
    }
}

impl Bulk {
    pub(crate) fn measure(skeleton: &Skeleton, rest: &Worlds, flesh: &[(HumanBone, Vec3)]) -> Self {
        let scale = skeleton.scale();
        let thigh = skeleton.frame(HumanBone::LeftUpperLeg).offset.length()
            + skeleton.frame(HumanBone::LeftLowerLeg).offset.length();
        let pieces = TRUNK
            .into_iter()
            .filter(|bone| skeleton.frame(*bone).present)
            .filter_map(|bone| {
                let frame = skeleton.frame(bone);
                let joint = rest.get(bone)?;
                let lowest = match bone {
                    HumanBone::Hips => -thigh,
                    _ => -INTO_PARENT * frame.offset.length(),
                };
                let points: Vec<Vec3> = flesh
                    .iter()
                    .filter(|(owner, _)| *owner == bone)
                    .map(|(_, point)| joint.local(*point))
                    .collect();
                Piece::measure(bone, frame.direction, lowest, scale, &points)
            })
            .collect();
        Self {
            pieces,
            stride: STRIDE * scale,
        }
    }

    #[must_use]
    pub fn holds(&self, worlds: &Worlds, point: Vec3, gap: f32) -> bool {
        self.pieces.iter().any(|piece| {
            worlds
                .get(piece.bone)
                .is_some_and(|joint| piece.holds(joint.local(point), gap))
        })
    }

    #[must_use]
    pub fn surface(&self, worlds: &Worlds, from: Vec3, toward: Vec3, gap: f32) -> Vec3 {
        let Some(toward) = toward.try_normalize() else {
            return from;
        };
        if !self.holds(worlds, from, gap) {
            return from;
        }
        let mut inside = from;
        for step in 1..=LONGEST_WALK {
            let ahead = from + toward * (self.stride * step as f32);
            if !self.holds(worlds, ahead, gap) {
                return self.edge(worlds, inside, ahead, gap);
            }
            inside = ahead;
        }
        from
    }

    fn edge(&self, worlds: &Worlds, inside: Vec3, outside: Vec3, gap: f32) -> Vec3 {
        let (mut inside, mut outside) = (inside, outside);
        for _ in 0..EDGE_STEPS {
            let middle = (inside + outside) * 0.5;
            match self.holds(worlds, middle, gap) {
                true => inside = middle,
                false => outside = middle,
            }
        }
        outside
    }
}
