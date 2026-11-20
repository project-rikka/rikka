use animate_body::{Body, Dof, HumanBone, Limb, Side, Worlds};
use rikka_model_core::glam::Vec3;

const LOWER_BODY: f32 = 0.3;
const UPPER_BODY: f32 = 0.55;
const SHOULDER: f32 = 0.6;
const SEGMENT: f32 = 0.5;
const PALM: f32 = 0.6;
const CROWN: f32 = 0.5;

const TORSO: [HumanBone; 3] = [HumanBone::Spine, HumanBone::Chest, HumanBone::UpperChest];
const NECK: [HumanBone; 2] = [HumanBone::Neck, HumanBone::Head];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Handle {
    Centre,
    LowerBody,
    UpperBody,
    Head,
    Shoulder(Side),
    UpperArm(Side),
    Forearm(Side),
    Hand(Side),
    Foot(Side),
    Toes(Side),
}

fn present(body: &Body, bones: &[HumanBone]) -> Vec<HumanBone> {
    bones
        .iter()
        .copied()
        .filter(|bone| body.has(*bone))
        .collect()
}

fn lerp_position(worlds: &Worlds, from: HumanBone, to: HumanBone, share: f32) -> Vec3 {
    worlds.position(from).lerp(worlds.position(to), share)
}

fn tip(body: &Body, worlds: &Worlds, bone: HumanBone, share: f32) -> Vec3 {
    worlds.point(bone, body.reach(bone).unwrap_or(Vec3::ZERO) * share)
}

fn toe_tip(body: &Body, worlds: &Worlds, side: Side) -> Vec3 {
    match body.has(side.toes()) {
        true => tip(body, worlds, side.toes(), 1.0),
        false => tip(body, worlds, side.foot(), 1.0),
    }
}

fn lap(body: &Body) -> Vec3 {
    let (sum, count) = Side::BOTH
        .into_iter()
        .map(|side| side.leg().chain())
        .filter(|[upper, lower, _]| body.has(*upper) && body.has(*lower))
        .fold((Vec3::ZERO, 0), |(sum, count), [upper, lower, _]| {
            (sum + body.offset(upper) + body.offset(lower), count + 1)
        });
    match count {
        0 => Vec3::ZERO,
        _ => sum / count as f32 * LOWER_BODY,
    }
}

fn neck_base(body: &Body) -> Option<HumanBone> {
    NECK.into_iter().find(|bone| body.has(*bone))
}

fn torso_top(body: &Body, worlds: &Worlds) -> Vec3 {
    neck_base(body).map_or_else(
        || tip(body, worlds, HumanBone::Spine, 1.0),
        |bone| worlds.position(bone),
    )
}

impl Handle {
    pub const ALL: [Self; 16] = [
        Self::Centre,
        Self::LowerBody,
        Self::UpperBody,
        Self::Head,
        Self::Shoulder(Side::Left),
        Self::Shoulder(Side::Right),
        Self::UpperArm(Side::Left),
        Self::UpperArm(Side::Right),
        Self::Forearm(Side::Left),
        Self::Forearm(Side::Right),
        Self::Hand(Side::Left),
        Self::Hand(Side::Right),
        Self::Foot(Side::Left),
        Self::Foot(Side::Right),
        Self::Toes(Side::Left),
        Self::Toes(Side::Right),
    ];

    #[must_use]
    pub fn side(&self) -> Option<Side> {
        match self {
            Self::Shoulder(side)
            | Self::UpperArm(side)
            | Self::Forearm(side)
            | Self::Hand(side)
            | Self::Foot(side)
            | Self::Toes(side) => Some(*side),
            Self::Centre | Self::LowerBody | Self::UpperBody | Self::Head => None,
        }
    }

    #[must_use]
    pub fn limb(&self) -> Option<Limb> {
        match self {
            Self::Shoulder(side)
            | Self::UpperArm(side)
            | Self::Forearm(side)
            | Self::Hand(side) => Some(side.arm()),
            Self::Foot(side) | Self::Toes(side) => Some(side.leg()),
            Self::Centre | Self::LowerBody | Self::UpperBody | Self::Head => None,
        }
    }

    #[must_use]
    pub fn end(&self) -> Option<Limb> {
        match self {
            Self::Hand(_) | Self::Foot(_) | Self::Toes(_) => self.limb(),
            _ => None,
        }
    }

    #[must_use]
    pub fn fits(&self, body: &Body) -> bool {
        let chain = |limb: Limb| limb.chain().iter().all(|bone| body.has(*bone));
        match self {
            Self::Centre => body.has(HumanBone::Hips),
            Self::LowerBody | Self::UpperBody => body.has(HumanBone::Spine),
            Self::Head => body.has(HumanBone::Head),
            Self::Shoulder(side) => body.has(side.shoulder()),
            Self::UpperArm(side) | Self::Forearm(side) | Self::Hand(side) => chain(side.arm()),
            Self::Foot(side) => chain(side.leg()),
            Self::Toes(side) => {
                body.has(side.foot())
                    && (body.has(side.toes()) || body.reach(side.foot()).is_some())
            }
        }
    }

    #[must_use]
    pub fn point(&self, body: &Body, worlds: &Worlds) -> Vec3 {
        match *self {
            Self::Centre => worlds.position(HumanBone::Hips),
            Self::LowerBody => worlds.point(HumanBone::Hips, lap(body)),
            Self::UpperBody => worlds
                .position(HumanBone::Spine)
                .lerp(torso_top(body, worlds), UPPER_BODY),
            Self::Head => tip(body, worlds, HumanBone::Head, CROWN),
            Self::Shoulder(side) => {
                lerp_position(worlds, side.shoulder(), side.arm().chain()[0], SHOULDER)
            }
            Self::UpperArm(side) => {
                let [upper, lower, _] = side.arm().chain();
                lerp_position(worlds, upper, lower, SEGMENT)
            }
            Self::Forearm(side) => {
                let [_, lower, hand] = side.arm().chain();
                lerp_position(worlds, lower, hand, SEGMENT)
            }
            Self::Hand(side) => tip(body, worlds, side.hand(), PALM),
            Self::Foot(side) => worlds.position(side.foot()),
            Self::Toes(side) => toe_tip(body, worlds, side),
        }
    }

    pub(crate) fn pivot(&self, body: &Body) -> HumanBone {
        match *self {
            Self::Centre | Self::LowerBody => HumanBone::Hips,
            Self::UpperBody => HumanBone::Spine,
            Self::Head => neck_base(body).unwrap_or(HumanBone::Head),
            Self::Shoulder(side) => side.shoulder(),
            Self::UpperArm(side) => side.arm().chain()[0],
            Self::Forearm(side) => side.arm().chain()[1],
            Self::Hand(side) => side.arm().chain()[2],
            Self::Foot(side) | Self::Toes(side) => side.foot(),
        }
    }

    pub(crate) fn turned(&self, body: &Body) -> Vec<HumanBone> {
        match *self {
            Self::UpperBody => present(body, &TORSO),
            Self::Head => present(body, &NECK),
            _ => vec![self.pivot(body)],
        }
    }

    pub(crate) fn twisted(&self, body: &Body) -> Vec<HumanBone> {
        let twisting = |bones: &[HumanBone]| {
            bones
                .iter()
                .copied()
                .filter(|bone| body.axis(*bone, Dof::Twist).is_some())
                .collect()
        };
        match *self {
            Self::Centre | Self::LowerBody | Self::Shoulder(_) | Self::Toes(_) => Vec::new(),
            Self::UpperBody => twisting(&TORSO),
            Self::Head => twisting(&NECK),
            Self::UpperArm(side) => twisting(&[side.arm().chain()[0]]),
            Self::Forearm(side) | Self::Hand(side) => twisting(&[side.arm().chain()[1]]),
            Self::Foot(side) => twisting(&[side.leg().chain()[0]]),
        }
    }

    pub(crate) fn turns_root(&self) -> bool {
        matches!(self, Self::Centre | Self::LowerBody)
    }

    pub(crate) fn keeps_torso(&self) -> bool {
        matches!(self, Self::LowerBody)
    }

    #[must_use]
    pub fn twist_axis(&self, body: &Body, worlds: &Worlds) -> Option<Vec3> {
        if self.turns_root() {
            return Some(Vec3::Y);
        }
        let bone = *self.twisted(body).first()?;
        Some(worlds.rotation(bone) * body.axis(bone, Dof::Twist)?)
    }

    pub(crate) fn carrier(&self) -> Option<HumanBone> {
        match self {
            Self::Hand(side) => Some(side.arm().chain()[1]),
            Self::Foot(side) => Some(side.leg().chain()[1]),
            _ => None,
        }
    }
}
