use rikka_physics_geometry::MassData;

use super::{Codec, Reader, Writer, get_list, put_list};
use crate::body::MassSpec;
use crate::{AxisLocks, Body, BodyKind, Error, Result, ShapeDef};

fn kind_tag(kind: BodyKind) -> u8 {
    match kind {
        BodyKind::Static => 0,
        BodyKind::Kinematic => 1,
        BodyKind::Dynamic => 2,
    }
}

fn kind_from(tag: u8) -> Result<BodyKind> {
    match tag {
        0 => Ok(BodyKind::Static),
        1 => Ok(BodyKind::Kinematic),
        2 => Ok(BodyKind::Dynamic),
        _ => Err(Error::Decode { what: "body kind" }),
    }
}

fn encode_mass(mass: MassSpec, out: &mut Writer<'_>) {
    match mass {
        MassSpec::FromShapes => out.tag(0),
        MassSpec::Total(total) => {
            out.tag(1);
            out.put(&total);
        }
        MassSpec::Explicit(data) => {
            out.tag(2);
            out.put(&data.mass);
            out.put(&data.center);
            out.put(&data.inertia);
        }
    }
}

fn decode_mass(input: &mut Reader<'_>) -> Result<MassSpec> {
    let what = "body mass";
    match input.tag()? {
        0 => Ok(MassSpec::FromShapes),
        1 => Ok(MassSpec::Total(input.get::<f32>()?)),
        2 => Ok(MassSpec::Explicit(MassData {
            mass: input.get::<f32>()?,
            center: input.get()?,
            inertia: input.get()?,
        })),
        _ => Err(Error::Decode { what }),
    }
}

impl Codec for Body {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        out.tag(kind_tag(self.kind));
        out.put(&self.pose);
        out.put(&self.linear_velocity);
        out.put(&self.angular_velocity);
        out.put(&self.linear_damping);
        out.put(&self.angular_damping);
        out.put(&self.gravity_scale);
        out.tag(self.locks.bits() as u8);
        out.put(&self.bullet);
        out.put(&self.sleep);
        out.put(&self.sleep_threshold);
        out.put(&self.awake);
        out.put(&self.character);
        out.put(&self.fast_rotation);
        out.put(&self.user_data);
        encode_mass(self.mass, out);
        put_list(out, &self.shapes);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self> {
        let what = "body";
        let kind = kind_from(input.tag()?)?;
        let pose = input.get()?;
        let linear_velocity = input.get()?;
        let angular_velocity = input.get()?;
        let linear_damping = input.get::<f32>()?;
        let angular_damping = input.get::<f32>()?;
        let gravity_scale = input.get::<f32>()?;
        let bits = input.tag()?;
        if bits >= 1 << 6 {
            return Err(Error::Decode { what });
        }
        let locks = AxisLocks {
            linear: [0, 1, 2].map(|axis| bits & (1 << axis) != 0),
            angular: [0, 1, 2].map(|axis| bits & (1 << (axis + 3)) != 0),
        };
        Ok(Self {
            kind,
            pose,
            linear_velocity,
            angular_velocity,
            linear_damping,
            angular_damping,
            gravity_scale,
            locks,
            bullet: input.get::<bool>()?,
            sleep: input.get::<bool>()?,
            sleep_threshold: input.get::<Option<f32>>()?,
            awake: input.get::<bool>()?,
            character: input.get::<bool>()?,
            fast_rotation: input.get::<bool>()?,
            user_data: input.get::<u64>()?,
            mass: decode_mass(input)?,
            shapes: get_list::<ShapeDef>(input)?,
        })
    }
}

impl Body {
    pub fn to_bytes(&self) -> Vec<u8> {
        super::to_bytes(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        super::from_bytes(bytes)
    }
}
