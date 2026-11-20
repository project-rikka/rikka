use super::{Codec, Reader, Writer};
use crate::joint::{Anchor, AxesParams, JointBase, JointParams};
use crate::{
    BodyId, D6, Distance, Drive, Error, FilterJoint, JointDef, Motor, Parallel, Prismatic, Result,
    Revolute, Spherical, Spring, Weld, Wheel,
};

impl Codec for Drive {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.spring);
        out.put(&self.max_effort);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            spring: input.get::<Option<Spring>>()?,
            max_effort: input.get::<f32>()?,
        })
    }
}

impl Codec for JointBase {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.body_a);
        out.put(&self.body_b);
        match self.anchor {
            Anchor::World(frame) => {
                out.tag(0);
                out.put(&frame);
            }
            Anchor::Local { a, b } => {
                out.tag(1);
                out.put(&a);
                out.put(&b);
            }
        }
        out.put(&self.collide_connected);
        out.put(&self.break_force);
        out.put(&self.break_torque);
        out.put(&self.force_threshold);
        out.put(&self.torque_threshold);
        out.put(&self.softness);
        out.put(&self.user_data);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self> {
        let what = "joint";
        let body_a = input.get::<BodyId>()?;
        let body_b = input.get::<BodyId>()?;
        let anchor = match input.tag()? {
            0 => Anchor::World(input.get()?),
            1 => Anchor::Local {
                a: input.get()?,
                b: input.get()?,
            },
            _ => return Err(Error::Decode { what }),
        };
        Ok(Self {
            body_a,
            body_b,
            anchor,
            collide_connected: input.get::<bool>()?,
            break_force: input.get::<f32>()?,
            break_torque: input.get::<f32>()?,
            force_threshold: input.get::<f32>()?,
            torque_threshold: input.get::<f32>()?,
            softness: input.get::<Option<Spring>>()?,
            user_data: input.get::<u64>()?,
        })
    }
}

macro_rules! tagged {
    ($name:ident, $what:literal, $($tag:literal => $variant:ident($inner:ty)),* $(,)?) => {
        impl Codec for $name {
            type Error = Error;

            fn encode(&self, out: &mut Writer<'_>) {
                match self {
                    $($name::$variant(inner) => {
                        out.tag($tag);
                        inner.encode(out);
                    })*
                }
            }

            fn decode(input: &mut Reader<'_>) -> Result<Self> {
                match input.tag()? {
                    $($tag => Ok($name::$variant(<$inner>::decode(input)?)),)*
                    _ => Err(Error::Decode { what: $what }),
                }
            }
        }
    };
}

tagged!(AxesParams, "axis joint",
    0 => D6(D6),
    1 => Revolute(Revolute),
    2 => Prismatic(Prismatic),
    3 => Wheel(Wheel),
    4 => Motor(Motor),
    5 => Parallel(Parallel),
);

tagged!(JointParams, "joint kind",
    0 => Spherical(Spherical),
    1 => Weld(Weld),
    2 => Axes(AxesParams),
    3 => Distance(Distance),
    4 => Filter(FilterJoint),
);

impl Codec for FilterJoint {
    type Error = Error;

    fn encode(&self, _: &mut Writer<'_>) {}

    fn decode(_: &mut Reader<'_>) -> Result<Self> {
        Ok(FilterJoint)
    }
}

impl Codec for JointDef {
    type Error = Error;

    fn encode(&self, out: &mut Writer<'_>) {
        self.base.encode(out);
        self.params.encode(out);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self> {
        Ok(Self {
            base: JointBase::decode(input)?,
            params: JointParams::decode(input)?,
        })
    }
}

impl JointDef {
    pub fn to_bytes(&self) -> Vec<u8> {
        super::to_bytes(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        super::from_bytes(bytes)
    }
}
