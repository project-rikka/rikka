use rikka_physics_math::{Aabb, Plane, Transform};

use crate::{
    Capsule, CastHit, Compound, Convex, ConvexHull, Cuboid, Cylinder, HeightField, MassData,
    MeshShape, ProxyBuffer, RayInput, Result, ShapeCastInput, ShapeProxy, Sphere,
};

pub trait ShapeGeometry {
    fn aabb(&self, transform: Transform) -> Aabb;
    fn mass(&self, density: f32) -> MassData;
    fn ray_cast(&self, ray: &RayInput) -> Option<CastHit>;
    fn shape_cast(&self, cast: &ShapeCastInput<'_>) -> Option<CastHit>;
    fn overlaps(&self, transform: Transform, proxy: &ShapeProxy<'_>) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ShapeKind {
    Sphere,
    Capsule,
    Cuboid,
    Cylinder,
    Hull,
    Mesh,
    HeightField,
    Compound,
    Plane,
}

#[derive(Clone, Debug)]
pub enum Shape {
    Sphere(Sphere),
    Capsule(Capsule),
    Cuboid(Cuboid),
    Cylinder(Cylinder),
    Hull(ConvexHull),
    Plane(Plane),
    Mesh(MeshShape),
    HeightField(HeightField),
    Compound(Compound),
}

macro_rules! each_shape {
    ($shape:expr, $inner:ident => $body:expr) => {
        match $shape {
            Shape::Sphere($inner) => $body,
            Shape::Capsule($inner) => $body,
            Shape::Cuboid($inner) => $body,
            Shape::Cylinder($inner) => $body,
            Shape::Hull($inner) => $body,
            Shape::Plane($inner) => $body,
            Shape::Mesh($inner) => $body,
            Shape::HeightField($inner) => $body,
            Shape::Compound($inner) => $body,
        }
    };
}

impl Shape {
    pub fn kind(&self) -> ShapeKind {
        match self {
            Self::Sphere(_) => ShapeKind::Sphere,
            Self::Capsule(_) => ShapeKind::Capsule,
            Self::Cuboid(_) => ShapeKind::Cuboid,
            Self::Cylinder(_) => ShapeKind::Cylinder,
            Self::Hull(_) => ShapeKind::Hull,
            Self::Plane(_) => ShapeKind::Plane,
            Self::Mesh(_) => ShapeKind::Mesh,
            Self::HeightField(_) => ShapeKind::HeightField,
            Self::Compound(_) => ShapeKind::Compound,
        }
    }

    pub fn as_convex(&self) -> Option<Convex<'_>> {
        match self {
            Self::Sphere(shape) => Some(Convex::Sphere(shape)),
            Self::Capsule(shape) => Some(Convex::Capsule(shape)),
            Self::Cuboid(shape) => Some(Convex::Cuboid(shape)),
            Self::Cylinder(shape) => Some(Convex::Cylinder(shape)),
            Self::Hull(shape) => Some(Convex::Hull(shape)),
            _ => None,
        }
    }

    pub fn proxy<'a>(&'a self, buffer: &'a mut ProxyBuffer) -> Option<ShapeProxy<'a>> {
        match self {
            Self::Sphere(shape) => Some(shape.proxy()),
            Self::Capsule(shape) => Some(shape.proxy(buffer)),
            Self::Cuboid(shape) => Some(shape.proxy(buffer)),
            Self::Cylinder(shape) => Some(shape.proxy(buffer)),
            Self::Hull(shape) => Some(shape.proxy()),
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Sphere(shape) => shape.validate(),
            Self::Capsule(shape) => shape.validate(),
            Self::Cuboid(shape) => shape.validate(),
            Self::Cylinder(shape) => shape.validate(),
            Self::Hull(shape) => shape.validate(),
            Self::Plane(plane) => crate::convex::validate_plane(plane),
            Self::Mesh(shape) => shape.validate(),
            Self::HeightField(_) | Self::Compound(_) => Ok(()),
        }
    }
}

impl ShapeGeometry for Shape {
    fn aabb(&self, transform: Transform) -> Aabb {
        each_shape!(self, shape => shape.aabb(transform))
    }

    fn mass(&self, density: f32) -> MassData {
        each_shape!(self, shape => shape.mass(density))
    }

    fn ray_cast(&self, ray: &RayInput) -> Option<CastHit> {
        each_shape!(self, shape => shape.ray_cast(ray))
    }

    fn shape_cast(&self, cast: &ShapeCastInput<'_>) -> Option<CastHit> {
        each_shape!(self, shape => shape.shape_cast(cast))
    }

    fn overlaps(&self, transform: Transform, proxy: &ShapeProxy<'_>) -> bool {
        each_shape!(self, shape => shape.overlaps(transform, proxy))
    }
}

impl From<Sphere> for Shape {
    fn from(shape: Sphere) -> Self {
        Self::Sphere(shape)
    }
}

impl From<Capsule> for Shape {
    fn from(shape: Capsule) -> Self {
        Self::Capsule(shape)
    }
}

impl From<Cuboid> for Shape {
    fn from(shape: Cuboid) -> Self {
        Self::Cuboid(shape)
    }
}

impl From<Cylinder> for Shape {
    fn from(shape: Cylinder) -> Self {
        Self::Cylinder(shape)
    }
}

impl From<ConvexHull> for Shape {
    fn from(shape: ConvexHull) -> Self {
        Self::Hull(shape)
    }
}

impl From<Plane> for Shape {
    fn from(shape: Plane) -> Self {
        Self::Plane(shape)
    }
}

impl From<MeshShape> for Shape {
    fn from(shape: MeshShape) -> Self {
        Self::Mesh(shape)
    }
}

impl From<HeightField> for Shape {
    fn from(shape: HeightField) -> Self {
        Self::HeightField(shape)
    }
}

impl From<Compound> for Shape {
    fn from(shape: Compound) -> Self {
        Self::Compound(shape)
    }
}
