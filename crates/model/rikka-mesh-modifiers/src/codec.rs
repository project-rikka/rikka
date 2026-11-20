use glam::{Vec2, Vec3};
use rikka_file_format::DecodeError;

macro_rules! tagged {
    ($name:ident, $what:literal { $($variant:ident = $tag:literal),* $(,)? }) => {
        impl rikka_file_format::Codec for $name {
            type Error = rikka_file_format::DecodeError;

            fn encode(&self, out: &mut rikka_file_format::Writer<'_>) {
                out.tag(match self {
                    $($name::$variant => $tag,)*
                });
            }

            fn decode(
                input: &mut rikka_file_format::Reader<'_>,
            ) -> Result<Self, rikka_file_format::DecodeError> {
                match input.tag()? {
                    $($tag => Ok($name::$variant),)*
                    _ => Err(rikka_file_format::DecodeError::Malformed(concat!("unknown ", $what))),
                }
            }
        }
    };
}

macro_rules! record {
    ($name:ident { $($field:ident $(=> $check:expr)?),* $(,)? }) => {
        impl rikka_file_format::Codec for $name {
            type Error = rikka_file_format::DecodeError;

            fn encode(&self, out: &mut rikka_file_format::Writer<'_>) {
                $(out.put(&self.$field);)*
            }

            fn decode(
                input: &mut rikka_file_format::Reader<'_>,
            ) -> Result<Self, rikka_file_format::DecodeError> {
                Ok(Self {
                    $($field: {
                        let value = input.get()?;
                        $(($check)(&value)?;)?
                        value
                    },)*
                })
            }
        }
    };
}

macro_rules! flags_codec {
    ($name:ident, $bits:ty, $what:literal) => {
        impl rikka_file_format::Codec for $name {
            type Error = rikka_file_format::DecodeError;

            fn encode(&self, out: &mut rikka_file_format::Writer<'_>) {
                out.put(&self.bits());
            }

            fn decode(
                input: &mut rikka_file_format::Reader<'_>,
            ) -> Result<Self, rikka_file_format::DecodeError> {
                $name::from_bits(input.get::<$bits>()?).ok_or(
                    rikka_file_format::DecodeError::Malformed(concat!("unknown ", $what)),
                )
            }
        }
    };
}

pub(crate) use {flags_codec, record, tagged};

pub(crate) fn finite(what: &'static str) -> impl Fn(&f32) -> Result<(), DecodeError> {
    move |value| match value.is_finite() {
        true => Ok(()),
        false => Err(DecodeError::Malformed(what)),
    }
}

pub(crate) fn within(
    low: f32,
    high: f32,
    what: &'static str,
) -> impl Fn(&f32) -> Result<(), DecodeError> {
    move |value| match value.is_finite() && (low..=high).contains(value) {
        true => Ok(()),
        false => Err(DecodeError::Malformed(what)),
    }
}

pub(crate) fn non_negative(what: &'static str) -> impl Fn(&f32) -> Result<(), DecodeError> {
    within(0.0, f32::MAX, what)
}

pub(crate) fn finite2(what: &'static str) -> impl Fn(&Vec2) -> Result<(), DecodeError> {
    move |value| match value.is_finite() {
        true => Ok(()),
        false => Err(DecodeError::Malformed(what)),
    }
}

pub(crate) fn within2(
    low: f32,
    high: f32,
    what: &'static str,
) -> impl Fn(&Vec2) -> Result<(), DecodeError> {
    move |value| match value.is_finite()
        && value.min_element() >= low
        && value.max_element() <= high
    {
        true => Ok(()),
        false => Err(DecodeError::Malformed(what)),
    }
}

pub(crate) fn finite3(what: &'static str) -> impl Fn(&Vec3) -> Result<(), DecodeError> {
    move |value| match value.is_finite() {
        true => Ok(()),
        false => Err(DecodeError::Malformed(what)),
    }
}

pub(crate) fn counted<T: PartialOrd + Copy>(
    low: T,
    high: T,
    what: &'static str,
) -> impl Fn(&T) -> Result<(), DecodeError> {
    move |value| match (low..=high).contains(value) {
        true => Ok(()),
        false => Err(DecodeError::Malformed(what)),
    }
}

pub(crate) fn items<T>(
    low: usize,
    high: usize,
    what: &'static str,
) -> impl Fn(&Vec<T>) -> Result<(), DecodeError> {
    move |values| match (low..=high).contains(&values.len()) {
        true => Ok(()),
        false => Err(DecodeError::Malformed(what)),
    }
}
