use rikka_humanoid::Anatomy;
use rikka_mmd::Puppet;
use rikka_model_core::glam::{Quat, Vec3};
use rikka_model_core::{HumanBone, Instance, Model, NodeId, Pose, Transform};

use crate::binding::Binding;
use crate::bulk::{Bulk, TRUNK};
use crate::error::MappingError;
use crate::flesh::flesh;
use crate::limb::Side;
use crate::lobe::{Lobe, lobes};
use crate::mapping::Mapping;
use crate::muscle::{Dof, muscles};
use crate::pose::HumanPose;
use crate::skeleton::Skeleton;
use crate::sole::Sole;
use crate::worlds::Worlds;

#[derive(Clone, Debug)]
pub struct Body {
    mapping: Mapping,
    skeleton: Skeleton,
    binding: Binding,
    rest_pose: HumanPose,
    soles: [Sole; 2],
    bulk: Bulk,
    lobes: Vec<Lobe>,
}

impl Body {
    pub fn from_anatomy(model: &Model, anatomy: &Anatomy) -> Result<Self, MappingError> {
        let mapping = Mapping::from_anatomy(model, anatomy)?;
        let rest = model.world_transforms();
        let skeleton = Skeleton::new(model, &mapping, &rest);
        let binding = Binding::new(model, &mapping, &rest);
        let rest_pose = binding.read(&skeleton, |node| rest[node.index()]);
        let still = skeleton.fk(&rest_pose);
        let folded = mapping.folded(model);
        let flesh = flesh(model, &folded, &rest, |bone| {
            TRUNK.contains(&bone)
                || Side::BOTH
                    .into_iter()
                    .any(|side| bone == side.foot() || bone == side.toes())
        });
        Ok(Self {
            soles: Side::BOTH.map(|side| Sole::measure(side, &still, &flesh)),
            bulk: Bulk::measure(&skeleton, &still, &flesh),
            lobes: lobes(model, &mapping, &folded, &rest, &still, skeleton.scale()),
            mapping,
            skeleton,
            binding,
            rest_pose,
        })
    }

    #[must_use]
    pub fn bulk(&self) -> &Bulk {
        &self.bulk
    }

    #[must_use]
    pub fn lobes(&self) -> &[Lobe] {
        &self.lobes
    }

    #[must_use]
    pub fn mapping(&self) -> &Mapping {
        &self.mapping
    }

    #[must_use]
    pub fn scale(&self) -> f32 {
        self.skeleton.scale()
    }

    #[must_use]
    pub fn rest_pose(&self) -> &HumanPose {
        &self.rest_pose
    }

    #[must_use]
    pub fn reach(&self, bone: HumanBone) -> Option<Vec3> {
        self.skeleton.frame(bone).reach
    }

    #[must_use]
    pub fn has(&self, bone: HumanBone) -> bool {
        self.skeleton.frame(bone).present
    }

    #[must_use]
    pub fn parent(&self, bone: HumanBone) -> Option<HumanBone> {
        self.skeleton.frame(bone).parent
    }

    #[must_use]
    pub fn offset(&self, bone: HumanBone) -> Vec3 {
        self.skeleton.frame(bone).offset
    }

    #[must_use]
    pub fn direction(&self, bone: HumanBone) -> Vec3 {
        self.skeleton.frame(bone).direction
    }

    #[must_use]
    pub fn axis(&self, bone: HumanBone, dof: Dof) -> Option<Vec3> {
        self.skeleton.axis(bone, dof)
    }

    #[must_use]
    pub fn local_rotation(&self, bone: HumanBone, pose: &HumanPose) -> Quat {
        self.skeleton.local_rotation(bone, pose)
    }

    pub fn set_local_rotation(&self, bone: HumanBone, rotation: Quat, pose: &mut HumanPose) {
        self.skeleton.set_local_rotation(bone, rotation, pose);
    }

    #[must_use]
    pub fn fk(&self, pose: &HumanPose) -> Worlds {
        self.skeleton.fk(pose)
    }

    #[must_use]
    pub fn fk_below(&self, base: &Worlds, pose: &HumanPose, moved: &[HumanBone]) -> Worlds {
        self.skeleton.fk_below(base, pose, moved)
    }

    #[must_use]
    pub fn driven_nodes(&self) -> &[NodeId] {
        self.binding.driven_nodes()
    }

    #[must_use]
    pub fn locals(&self, pose: &HumanPose) -> Vec<Transform> {
        self.binding.locals(&self.skeleton, pose)
    }

    pub fn write(&self, pose: &HumanPose, target: &mut Pose) {
        for (node, local) in self.driven_nodes().iter().zip(self.locals(pose)) {
            target.set_local(*node, local);
        }
    }

    pub fn write_fingers(&self, pose: &HumanPose, sides: &[Side], target: &mut Pose) {
        if sides.is_empty() {
            return;
        }
        let fingered =
            |bone: HumanBone| sides.iter().any(|side| side.finger_bones().contains(&bone));
        for (node, local) in self.driven_nodes().iter().zip(self.locals(pose)) {
            if self.mapping.bone_of(*node).is_some_and(fingered) {
                target.set_local(*node, local);
            }
        }
    }

    pub fn release(&self, puppet: &mut Puppet) {
        for bone in self.mapping.reaching() {
            puppet.set_reaching(*bone, false);
        }
    }

    #[must_use]
    pub fn read(&self, instance: &Instance) -> HumanPose {
        self.binding
            .read(&self.skeleton, |node| instance.world(node))
    }

    pub fn clamp(&self, pose: &mut HumanPose, limits: bool) {
        for (value, muscle) in pose.muscles.iter_mut().zip(muscles()) {
            *value = muscle.keep(*value, limits);
        }
        self.skeleton.shorten_swings(pose);
    }

    pub fn clamp_around(&self, pose: &mut HumanPose, around: &HumanPose, limits: bool) {
        for ((value, start), muscle) in pose.muscles.iter_mut().zip(around.muscles).zip(muscles()) {
            let (low, high) = muscle.bounds_around(limits, start);
            *value = value.clamp(low, high);
        }
        self.skeleton.shorten_swings(pose);
    }

    #[must_use]
    pub fn sole(&self, side: Side) -> Sole {
        match side {
            Side::Left => self.soles[0],
            Side::Right => self.soles[1],
        }
    }

    #[must_use]
    pub fn sole_height(&self, worlds: &Worlds, side: Side) -> f32 {
        let sole = self.sole(side);
        let heel = worlds.point(side.foot(), sole.heel);
        let toe = worlds.point(side.toes(), sole.toe);
        heel.y.min(toe.y)
    }
}
