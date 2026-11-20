use bytemuck::{Pod, Zeroable};
use rikka_physics_math::{Quat, Transform, Vec3};

use crate::{Capsule, Cuboid, Cylinder, Shape, Sphere};

pub(crate) const SPHERE: u32 = 0;
pub(crate) const CAPSULE: u32 = 1;
pub(crate) const CUBOID: u32 = 2;
pub(crate) const CYLINDER: u32 = 3;
pub(crate) const HULL: u32 = 4;
pub(crate) const MESH: u32 = 5;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub(crate) struct ChildRecord {
    pub(crate) position: Vec3,
    pub(crate) rotation: [f32; 4],
    pub(crate) kind: u32,
    pub(crate) material: u32,
    pub(crate) blob: u32,
    pub(crate) params: [f32; 12],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Pod, Zeroable)]
pub(crate) struct BlobRecord {
    pub(crate) offset: u32,
    pub(crate) len: u32,
}

impl ChildRecord {
    #[inline]
    pub(crate) fn transform(&self) -> Transform {
        Transform::new(self.position, Quat::from_array(self.rotation))
    }

    pub(crate) fn value(transform: Transform, material: u32, shape: &Shape) -> Option<Self> {
        let mut record = Self {
            position: transform.p,
            rotation: transform.q.to_array(),
            material,
            blob: u32::MAX,
            ..Self::default()
        };
        let params = &mut record.params;
        record.kind = match shape {
            Shape::Sphere(sphere) => {
                params[..3].copy_from_slice(&sphere.center.to_array());
                params[3] = sphere.radius;
                SPHERE
            }
            Shape::Capsule(capsule) => {
                params[..3].copy_from_slice(&capsule.center1.to_array());
                params[3] = capsule.radius1;
                params[4..7].copy_from_slice(&capsule.center2.to_array());
                params[7] = capsule.radius2;
                CAPSULE
            }
            Shape::Cuboid(cuboid) => {
                params[..3].copy_from_slice(&cuboid.transform.p.to_array());
                params[3..7].copy_from_slice(&cuboid.transform.q.to_array());
                params[7..10].copy_from_slice(&cuboid.half_extents.to_array());
                params[10] = cuboid.radius;
                CUBOID
            }
            Shape::Cylinder(cylinder) => {
                params[..3].copy_from_slice(&cylinder.transform.p.to_array());
                params[3..7].copy_from_slice(&cylinder.transform.q.to_array());
                params[7] = cylinder.half_height;
                params[8] = cylinder.radius;
                CYLINDER
            }
            _ => return None,
        };
        Some(record)
    }

    pub(crate) fn decode_value(&self) -> Option<Shape> {
        let p = &self.params;
        let vector = |at: usize| Vec3::new(p[at], p[at + 1], p[at + 2]);
        let transform = || Transform::new(vector(0), Quat::from_array([p[3], p[4], p[5], p[6]]));
        Some(match self.kind {
            SPHERE => Shape::Sphere(Sphere::with_center(vector(0), p[3])),
            CAPSULE => Shape::Capsule(Capsule::tapered(vector(0), p[3], vector(4), p[7])),
            CUBOID => Shape::Cuboid(Cuboid::with_transform(transform(), vector(7)).rounded(p[10])),
            CYLINDER => Shape::Cylinder(Cylinder::with_transform(transform(), p[7], p[8])),
            _ => return None,
        })
    }
}
