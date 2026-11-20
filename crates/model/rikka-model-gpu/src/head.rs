use rikka_humanoid::Anatomy;
use rikka_model_core::glam::{Mat4, Quat, Vec3};
use rikka_model_core::{HumanBone, Instance, Model, NodeId, Transform};
use rikka_renderer_3d::Head;

const HEAD_LIFT: f32 = 0.08;
const HEAD_RADIUS: f32 = 0.1;
const SMALLEST_HIPS_HEIGHT: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadRest {
    node: NodeId,
    unturn: Quat,
    forward: Vec3,
    scale: f32,
}

impl HeadRest {
    #[must_use]
    pub fn new(model: &Model, anatomy: &Anatomy) -> Option<Self> {
        let head = anatomy.node(HumanBone::Head)?;
        let rest = model.world_transforms();
        let scale = anatomy
            .node(HumanBone::Hips)
            .and_then(|hips| rest.get(hips.index()))
            .map(|hips| hips.w_axis.y)
            .filter(|height| *height > SMALLEST_HIPS_HEIGHT)
            .unwrap_or(1.0);
        Some(Self {
            node: head,
            unturn: rotation(*rest.get(head.index())?).inverse(),
            forward: anatomy.forward(),
            scale,
        })
    }

    #[must_use]
    pub fn place(&self, instance: &Instance) -> Head {
        let world = instance.world(self.node);
        let turn = rotation(world) * self.unturn;
        let up = turn * Vec3::Y;
        let center = world.w_axis.truncate() + up * HEAD_LIFT * self.scale;
        let placement = instance.transform();
        let placed_up = placement.transform_vector3(up);
        Head {
            center: placement.transform_point3(center),
            forward: placement
                .transform_vector3(turn * self.forward)
                .normalize_or(Vec3::Z),
            up: placed_up.normalize_or(Vec3::Y),
            radius: HEAD_RADIUS * self.scale * placed_up.length(),
        }
    }
}

fn rotation(world: Mat4) -> Quat {
    Transform::from_matrix(world).rotation
}
