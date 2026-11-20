use rikka_file_format::{Codec, DecodeError, Reader, Writer};

use crate::{Filter, Material, Mover, QueryFilter};

impl Codec for Material {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.friction);
        out.put(&self.restitution);
        out.put(&self.rolling_resistance);
        out.put(&self.tangent_velocity);
        out.put(&self.user_id);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            friction: input.get()?,
            restitution: input.get()?,
            rolling_resistance: input.get()?,
            tangent_velocity: input.get()?,
            user_id: input.get()?,
        })
    }
}

impl Codec for Filter {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.category);
        out.put(&self.mask);
        out.put(&self.group);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Self {
            category: input.get()?,
            mask: input.get()?,
            group: input.get()?,
        })
    }
}

impl Codec for QueryFilter {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.category);
        out.put(&self.mask);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(QueryFilter::default()
            .category(input.get()?)
            .mask(input.get()?))
    }
}

impl Codec for Mover {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.center1);
        out.put(&self.center2);
        out.put(&self.radius);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Mover::new(input.get()?, input.get()?, input.get()?))
    }
}
