use rikka_physics_collision::{Filter, Material};
use rikka_physics_core::Blob;
use rikka_physics_geometry::{
    Capsule, Compound, ConvexHull, Cuboid, Cylinder, HeightField, MeshShape, Shape, Sphere,
    TriangleMesh,
};
use rikka_physics_math::Plane;

use super::{Codec, Reader, Writer, get_list, put_list};
use crate::{Error, Result, ShapeDef};

const SPHERE: u8 = 0;
const CAPSULE: u8 = 1;
const CUBOID: u8 = 2;
const CYLINDER: u8 = 3;
const HULL: u8 = 4;
const PLANE: u8 = 5;
const MESH: u8 = 6;
const HEIGHT_FIELD: u8 = 7;
const COMPOUND: u8 = 8;

fn encode_shape(shape: &Shape, out: &mut Writer<'_>) {
    match shape {
        Shape::Sphere(sphere) => {
            out.tag(SPHERE);
            out.put(&sphere.center);
            out.put(&sphere.radius);
        }
        Shape::Capsule(capsule) => {
            out.tag(CAPSULE);
            out.put(&capsule.center1);
            out.put(&capsule.center2);
            out.put(&capsule.radius1);
            out.put(&capsule.radius2);
        }
        Shape::Cuboid(cuboid) => {
            out.tag(CUBOID);
            out.put(&cuboid.transform);
            out.put(&cuboid.half_extents);
            out.put(&cuboid.radius);
        }
        Shape::Cylinder(cylinder) => {
            out.tag(CYLINDER);
            out.put(&cylinder.transform);
            out.put(&cylinder.half_height);
            out.put(&cylinder.radius);
        }
        Shape::Hull(hull) => {
            out.tag(HULL);
            out.bytes(hull.blob().bytes());
            out.put(&hull.radius());
        }
        Shape::Plane(plane) => {
            out.tag(PLANE);
            out.put(&plane.normal);
            out.put(&plane.offset);
        }
        Shape::Mesh(mesh) => {
            out.tag(MESH);
            out.bytes(mesh.mesh.bytes());
            out.put(&mesh.scale);
        }
        Shape::HeightField(field) => {
            out.tag(HEIGHT_FIELD);
            out.bytes(field.bytes());
        }
        Shape::Compound(compound) => {
            out.tag(COMPOUND);
            out.bytes(compound.bytes());
        }
    }
}

fn decode_shape(input: &mut Reader<'_>) -> Result<Shape> {
    let what = "shape";
    let shape = match input.tag()? {
        SPHERE => Shape::Sphere(Sphere {
            center: input.get()?,
            radius: input.get::<f32>()?,
        }),
        CAPSULE => Shape::Capsule(Capsule {
            center1: input.get()?,
            center2: input.get()?,
            radius1: input.get::<f32>()?,
            radius2: input.get::<f32>()?,
        }),
        CUBOID => Shape::Cuboid(Cuboid {
            transform: input.get()?,
            half_extents: input.get()?,
            radius: input.get::<f32>()?,
        }),
        CYLINDER => Shape::Cylinder(Cylinder {
            transform: input.get()?,
            half_height: input.get::<f32>()?,
            radius: input.get::<f32>()?,
        }),
        HULL => {
            let blob = Blob::from_bytes(input.bytes()?).map_err(|_| Error::Decode { what })?;
            Shape::Hull(ConvexHull::from_blob(blob)?.rounded(input.get::<f32>()?))
        }
        PLANE => Shape::Plane(Plane::new(input.get()?, input.get::<f32>()?)),
        MESH => {
            let mesh = TriangleMesh::from_bytes(input.bytes()?)?;
            Shape::Mesh(MeshShape::scaled(mesh, input.get()?))
        }
        HEIGHT_FIELD => Shape::HeightField(HeightField::from_bytes(input.bytes()?)?),
        COMPOUND => Shape::Compound(Compound::from_bytes(input.bytes()?)?),
        _ => return Err(Error::Decode { what }),
    };
    shape.validate()?;
    Ok(shape)
}

impl Codec for ShapeDef {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        encode_shape(&self.shape, out);
        put_list(out, &self.materials);
        out.put(&self.filter);
        out.put(&self.sensor);
        out.put(&self.density);
        out.put(&self.contact_events);
        out.put(&self.hit_events);
        out.put(&self.sensor_events);
        out.put(&self.contact_response);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self> {
        let what = "shape definition";
        let shape = decode_shape(input)?;
        let materials: Vec<Material> = get_list(input)?;
        if materials.is_empty() {
            return Err(Error::Decode { what });
        }
        Ok(Self {
            shape,
            materials,
            filter: input.get::<Filter>()?,
            sensor: input.get::<bool>()?,
            density: input.get::<f32>()?,
            contact_events: input.get::<bool>()?,
            hit_events: input.get::<bool>()?,
            sensor_events: input.get::<bool>()?,
            contact_response: input.get::<bool>()?,
        })
    }
}

impl ShapeDef {
    pub fn to_bytes(&self) -> Vec<u8> {
        super::to_bytes(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        super::from_bytes(bytes)
    }
}
