use std::fmt;
use std::hash::Hash;

use crate::error::Result;
use crate::interface::Interface;
use crate::message::Args;
use crate::object::Object;
use crate::table::Objects;

pub trait Resource: Clone + Eq + Hash + fmt::Debug + Sized + 'static {
    type Request<'m>: fmt::Debug;

    const INTERFACE: &'static Interface;

    fn from_object(object: Object) -> Option<Self>;

    fn object(&self) -> &Object;

    fn decode<'m>(
        parent: &Object,
        opcode: u16,
        args: &mut Args<'m>,
        objects: &mut Objects<'_>,
    ) -> Result<Self::Request<'m>>;
}

pub trait Bindable: Resource {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UnknownValue(pub u32);

impl fmt::Display for UnknownValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown value {}", self.0)
    }
}

pub fn raw_value<T: Into<u32>>(value: Result<T, UnknownValue>) -> u32 {
    match value {
        Ok(value) => value.into(),
        Err(UnknownValue(raw)) => raw,
    }
}
