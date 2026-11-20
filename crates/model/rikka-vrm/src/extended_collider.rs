use rikka_model_core::glam::Vec3;

use rikka_model_core::Conversion;
use rikka_model_core::Extension;

use super::spring::ColliderShape;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ExtendedShape {
    Sphere {
        offset: Vec3,
        radius: f32,
        inside: bool,
    },
    Capsule {
        offset: Vec3,
        radius: f32,
        tail: Vec3,
        inside: bool,
    },
    Plane {
        offset: Vec3,
        normal: Vec3,
    },
}

impl ExtendedShape {
    pub const PLANE_FALLBACK_RADIUS: f32 = 1000.0;

    pub const INSIDE_FALLBACK_OFFSET: Vec3 = Vec3::new(0.0, -10000.0, 0.0);

    #[must_use]
    pub fn fallback(&self) -> ColliderShape {
        match *self {
            Self::Sphere {
                offset,
                radius,
                inside: false,
            } => ColliderShape::Sphere { offset, radius },
            Self::Capsule {
                offset,
                radius,
                tail,
                inside: false,
            } => ColliderShape::Capsule {
                offset,
                radius,
                tail,
            },
            Self::Sphere { inside: true, .. } | Self::Capsule { inside: true, .. } => {
                ColliderShape::Sphere {
                    offset: Self::INSIDE_FALLBACK_OFFSET,
                    radius: 0.0,
                }
            }
            Self::Plane { offset, normal } => ColliderShape::Sphere {
                offset: offset - normal.normalize_or(Vec3::Z) * Self::PLANE_FALLBACK_RADIUS,
                radius: Self::PLANE_FALLBACK_RADIUS,
            },
        }
    }

    fn convert(&mut self, conversion: &Conversion) {
        match self {
            Self::Sphere { offset, radius, .. } => {
                *offset = conversion.point(*offset);
                *radius *= conversion.scale();
            }
            Self::Capsule {
                offset,
                radius,
                tail,
                ..
            } => {
                *offset = conversion.point(*offset);
                *tail = conversion.point(*tail);
                *radius *= conversion.scale();
            }
            Self::Plane { offset, normal } => {
                *offset = conversion.point(*offset);
                *normal = conversion.vector(*normal).normalize_or(Vec3::Z);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtendedCollider {
    pub collider: usize,
    pub shape: ExtendedShape,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtendedColliders {
    pub colliders: Vec<ExtendedCollider>,
}

impl Extension for ExtendedColliders {
    const NAME: &'static str = "VRMC_springBone_extended_collider";

    fn convert(&mut self, conversion: &Conversion) {
        for collider in &mut self.colliders {
            collider.shape.convert(conversion);
        }
    }
}

impl ExtendedColliders {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.colliders.is_empty()
    }

    #[must_use]
    pub fn shape_of(&self, collider: usize) -> Option<&ExtendedShape> {
        self.colliders
            .iter()
            .find(|extended| extended.collider == collider)
            .map(|extended| &extended.shape)
    }
}
