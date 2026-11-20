mod audio;
mod connection;
mod fields;
mod numbers;
mod rates;
mod sample;
mod worker;

use std::io::Cursor;

use libspa::param::ParamType;
use libspa::pod::serialize::PodSerializer;
use libspa::pod::{ChoiceValue, Object, Property, Value};
use libspa::sys;
use libspa::utils::{Choice, ChoiceEnum, Id, Rectangle, SpaTypes};

pub use audio::float_format;
pub use connection::Connection;
pub use fields::Fields;
pub use numbers::{number, numbers};
pub use rates::{OfferedRates, RATES, rates_within};
pub use sample::{RawFormat, Sample};
pub use worker::{LoopStop, LoopWorker, spawn};

#[must_use]
pub fn object(kind: SpaTypes, id: ParamType, properties: Vec<Property>) -> Vec<u8> {
    let value = Value::Object(Object {
        type_: kind.as_raw(),
        id: id.as_raw(),
        properties,
    });
    PodSerializer::serialize(Cursor::new(Vec::new()), &value)
        .expect("pod built from typed values serializes")
        .0
        .into_inner()
}

#[must_use]
pub fn meta(kind: u32, size: usize) -> Vec<u8> {
    object(
        SpaTypes::ObjectParamMeta,
        ParamType::Meta,
        vec![
            Property::new(sys::SPA_PARAM_META_type, Value::Id(Id(kind))),
            Property::new(
                sys::SPA_PARAM_META_size,
                Value::Int(i32::try_from(size).unwrap_or(i32::MAX)),
            ),
        ],
    )
}

#[must_use]
pub fn default_id(value: &Value) -> Option<u32> {
    match value {
        Value::Id(Id(id))
        | Value::Choice(ChoiceValue::Id(Choice(
            _,
            ChoiceEnum::None(Id(id))
            | ChoiceEnum::Enum {
                default: Id(id), ..
            },
        ))) => Some(*id),
        _ => None,
    }
}

#[must_use]
pub fn default_rectangle(value: &Value) -> Option<Rectangle> {
    match value {
        Value::Rectangle(size) => Some(*size),
        Value::Choice(ChoiceValue::Rectangle(Choice(_, choice))) => match choice {
            ChoiceEnum::None(size)
            | ChoiceEnum::Enum { default: size, .. }
            | ChoiceEnum::Range { default: size, .. }
            | ChoiceEnum::Step { default: size, .. }
            | ChoiceEnum::Flags { default: size, .. } => Some(*size),
        },
        _ => None,
    }
}
