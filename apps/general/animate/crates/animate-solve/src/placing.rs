use animate_body::{Body, Dof, HumanBone, HumanPose, Worlds, muscle_of};
use rikka_model_core::glam::Quat;

pub(crate) struct Placing<'a> {
    body: &'a Body,
    around: &'a HumanPose,
    limits: bool,
    before: Worlds,
    pose: HumanPose,
    placed: Vec<(HumanBone, Quat)>,
}

impl<'a> Placing<'a> {
    pub(crate) fn new(
        body: &'a Body,
        pose: HumanPose,
        around: &'a HumanPose,
        limits: bool,
    ) -> Self {
        Self {
            before: body.fk(&pose),
            body,
            around,
            limits,
            pose,
            placed: Vec::new(),
        }
    }

    pub(crate) fn before(&self) -> &Worlds {
        &self.before
    }

    fn world(&self, bone: HumanBone) -> Quat {
        self.placed
            .iter()
            .rev()
            .find(|(placed, _)| *placed == bone)
            .map_or_else(|| self.before.rotation(bone), |(_, world)| *world)
    }

    pub(crate) fn place(&mut self, bone: HumanBone, desired: Quat) {
        self.put(bone, desired, false);
    }

    pub(crate) fn swing(&mut self, bone: HumanBone, desired: Quat) {
        self.put(bone, desired, true);
    }

    fn put(&mut self, bone: HumanBone, desired: Quat, untwisted: bool) {
        let body = self.body;
        let above = body
            .parent(bone)
            .filter(|_| bone != HumanBone::Hips)
            .map_or(Quat::IDENTITY, |parent| self.world(parent));
        let twist = muscle_of(bone, Dof::Twist).map(|index| (index, self.pose.muscles[index]));
        body.set_local_rotation(
            bone,
            (above.inverse() * desired).normalize(),
            &mut self.pose,
        );
        if let Some((index, twist_value)) = twist.filter(|_| untwisted) {
            self.pose.muscles[index] = twist_value;
        }
        body.clamp_around(&mut self.pose, self.around, self.limits);
        let world = (above * body.local_rotation(bone, &self.pose)).normalize();
        self.placed.push((bone, world));
    }

    pub(crate) fn finish(self) -> HumanPose {
        self.pose
    }
}
