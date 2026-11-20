use std::fmt;

pub struct Interface {
    pub index: u16,
    pub name: &'static str,
    pub version: u32,
    pub requests: &'static [MessageSpec],
    pub events: &'static [MessageSpec],
}

impl Interface {
    pub fn is(&self, other: &Interface) -> bool {
        self.index == other.index
    }
}

impl fmt::Debug for Interface {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Interface")
            .field("name", &self.name)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct MessageSpec {
    pub name: &'static str,
    pub since: u32,
    pub destructor: bool,
    pub signature: Signature,
    pub children: &'static [&'static Interface],
}

pub type Signature = &'static [ArgType];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    Int,
    Uint,
    Fixed,
    String,
    NullableString,
    Object,
    NullableObject,
    NewId,
    UntypedNewId,
    Array,
    Fd,
}

impl ArgType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Uint => "uint",
            Self::Fixed => "fixed",
            Self::String => "string",
            Self::NullableString => "nullable string",
            Self::Object => "object",
            Self::NullableObject => "nullable object",
            Self::NewId => "new_id",
            Self::UntypedNewId => "untyped new_id",
            Self::Array => "array",
            Self::Fd => "fd",
        }
    }
}
