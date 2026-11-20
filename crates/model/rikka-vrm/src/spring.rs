use rikka_model_core::glam::Vec3;

use rikka_model_core::Conversion;
use rikka_model_core::NodeId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColliderShape {
    Sphere {
        offset: Vec3,
        radius: f32,
    },
    Capsule {
        offset: Vec3,
        radius: f32,
        tail: Vec3,
    },
}

impl ColliderShape {
    fn convert(&mut self, conversion: &Conversion) {
        match self {
            Self::Sphere { offset, radius } => {
                *offset = conversion.point(*offset);
                *radius *= conversion.scale();
            }
            Self::Capsule {
                offset,
                radius,
                tail,
            } => {
                *offset = conversion.point(*offset);
                *tail = conversion.point(*tail);
                *radius *= conversion.scale();
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Collider {
    pub node: NodeId,
    pub shape: ColliderShape,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColliderGroup {
    pub name: String,
    pub colliders: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringJoint {
    pub node: NodeId,
    pub hit_radius: f32,
    pub stiffness: f32,
    pub gravity_power: f32,
    pub gravity_direction: Vec3,
    pub drag: f32,
}

impl Default for SpringJoint {
    fn default() -> Self {
        Self {
            node: NodeId(0),
            hit_radius: 0.0,
            stiffness: 1.0,
            gravity_power: 0.0,
            gravity_direction: Vec3::NEG_Y,
            drag: 0.5,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spring {
    pub name: String,
    pub joints: Vec<SpringJoint>,
    pub collider_groups: Vec<usize>,
    pub center: Option<NodeId>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpringBone {
    pub colliders: Vec<Collider>,
    pub collider_groups: Vec<ColliderGroup>,
    pub springs: Vec<Spring>,
}

impl SpringBone {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.springs.is_empty() && self.colliders.is_empty()
    }

    pub fn colliders_of<'a>(&'a self, spring: &'a Spring) -> impl Iterator<Item = &'a Collider> {
        spring
            .collider_groups
            .iter()
            .filter_map(|group| self.collider_groups.get(*group))
            .flat_map(|group| group.colliders.iter())
            .filter_map(|collider| self.colliders.get(*collider))
    }

    pub(crate) fn convert(&mut self, conversion: &Conversion) {
        for collider in &mut self.colliders {
            collider.shape.convert(conversion);
        }
        for spring in &mut self.springs {
            for joint in &mut spring.joints {
                joint.hit_radius *= conversion.scale();
                joint.gravity_direction = conversion
                    .vector(joint.gravity_direction)
                    .normalize_or(Vec3::NEG_Y);
            }
        }
    }
}
