use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};

use crate::codec::{Codec, Reader, Writer};
use crate::error::DecodeError;

macro_rules! number {
    ($($ty:ty),*) => {
        $(
            impl Codec for $ty {
                type Error = DecodeError;

                fn encode(&self, out: &mut Writer<'_>) {
                    out.raw(&self.to_le_bytes());
                }

                fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
                    input.array().map(<$ty>::from_le_bytes)
                }
            }
        )*
    };
}

number!(u8, u16, u32, u64, u128, i8, i16, i32, i64, f32, f64);

impl Codec for bool {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.tag(u8::from(*self));
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        match input.tag()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DecodeError::Malformed("boolean flag is not 0 or 1")),
        }
    }
}

impl<T: Codec> Codec for Option<T> {
    type Error = T::Error;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.is_some());
        if let Some(value) = self {
            out.put(value);
        }
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, T::Error> {
        match input.get::<bool>()? {
            true => input.get::<T>().map(Some),
            false => Ok(None),
        }
    }
}

impl<T: Codec> Codec for Vec<T> {
    type Error = T::Error;

    fn encode(&self, out: &mut Writer<'_>) {
        out.count64(self.len());
        out.put_all(self);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, T::Error> {
        let count = input.count64(1)?;
        (0..count).map(|_| input.get()).collect()
    }
}

impl<A: Codec, B: Codec<Error = A::Error>> Codec for (A, B) {
    type Error = A::Error;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.0);
        out.put(&self.1);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, A::Error> {
        let first = input.get::<A>()?;
        let second = input.get::<B>()?;
        Ok((first, second))
    }
}

impl<T: Codec, const N: usize> Codec for [T; N] {
    type Error = T::Error;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put_all(self);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, T::Error> {
        let mut values = Vec::<T>::with_capacity(N);
        for _ in 0..N {
            values.push(input.get::<T>()?);
        }
        Ok(values
            .try_into()
            .unwrap_or_else(|_| unreachable!("exactly N values were decoded")))
    }
}

impl Codec for String {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.str(self);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        input.str().map(str::to_owned)
    }
}

impl Codec for Vec2 {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put_all(&self.to_array());
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Vec2::new(input.get()?, input.get()?))
    }
}

impl Codec for Vec3 {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put_all(&self.to_array());
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Vec3::new(input.get()?, input.get()?, input.get()?))
    }
}

impl Codec for Vec4 {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put_all(&self.to_array());
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Vec4::new(
            input.get()?,
            input.get()?,
            input.get()?,
            input.get()?,
        ))
    }
}

impl Codec for Quat {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put_all(&self.to_array());
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Quat::from_xyzw(
            input.get()?,
            input.get()?,
            input.get()?,
            input.get()?,
        ))
    }
}

impl Codec for Mat3 {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.x_axis);
        out.put(&self.y_axis);
        out.put(&self.z_axis);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Mat3::from_cols(input.get()?, input.get()?, input.get()?))
    }
}

impl Codec for Mat4 {
    type Error = DecodeError;

    fn encode(&self, out: &mut Writer<'_>) {
        out.put(&self.x_axis);
        out.put(&self.y_axis);
        out.put(&self.z_axis);
        out.put(&self.w_axis);
    }

    fn decode(input: &mut Reader<'_>) -> Result<Self, DecodeError> {
        Ok(Mat4::from_cols(
            input.get()?,
            input.get()?,
            input.get()?,
            input.get()?,
        ))
    }
}
