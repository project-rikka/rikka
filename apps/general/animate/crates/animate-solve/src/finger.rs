use animate_body::{Body, Dof, HumanBone, HumanPose, Side, Worlds, muscles, muscles_of};
use rikka_model_core::glam::Vec3;

const JOINTS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Finger {
    side: Side,
    digit: usize,
}

impl Finger {
    pub const DIGITS: usize = 5;

    #[must_use]
    pub fn all(side: Side) -> [Self; Self::DIGITS] {
        std::array::from_fn(|digit| Self { side, digit })
    }

    #[must_use]
    pub fn side(&self) -> Side {
        self.side
    }

    #[must_use]
    pub fn digit(&self) -> usize {
        self.digit
    }

    fn bones(&self) -> [HumanBone; JOINTS] {
        let bones = self.side.finger_bones();
        std::array::from_fn(|joint| bones[self.digit * JOINTS + joint])
    }

    fn tip_bone(&self) -> HumanBone {
        self.bones()[JOINTS - 1]
    }

    #[must_use]
    pub fn fits(&self, body: &Body) -> bool {
        body.has(self.tip_bone())
    }

    #[must_use]
    pub fn tip(&self, body: &Body, worlds: &Worlds) -> Vec3 {
        let bone = self.tip_bone();
        worlds.point(bone, body.reach(bone).unwrap_or(Vec3::ZERO))
    }

    #[must_use]
    pub fn flex(
        &self,
        body: &Body,
        pose: &HumanPose,
        curl: f32,
        spread: f32,
        limits: bool,
    ) -> HumanPose {
        let mut flexed = *pose;
        for index in self.bones().into_iter().flat_map(muscles_of) {
            flexed.muscles[index] += match muscles()[index].dof {
                Dof::Side => spread,
                _ => curl,
            };
        }
        body.clamp_around(&mut flexed, pose, limits);
        flexed
    }
}
