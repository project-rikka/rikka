use rikka_file_format::{Codec, DecodeError, Reader, Writer};

use crate::{Aabb, Transform};

impl Codec for Transform {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.p);
        out.put(&self.q);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Transform::new(input.get()?, input.get()?))
    }
}

impl Codec for Aabb {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.min);
        out.put(&self.max);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Aabb::new(input.get()?, input.get()?))
    }
}
