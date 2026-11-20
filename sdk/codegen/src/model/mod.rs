mod docs;
mod resolve;
mod symbols;

use std::fmt;
use std::marker::PhantomData;
use std::ops::Index;

use crate::diagnostic::Span;

pub(crate) use docs::{Doc, Paragraph, Target};
pub(crate) use resolve::resolve;

pub(crate) use crate::syntax::ast::{DeclarationKind, Lifetime};

pub(crate) struct Id<T> {
    index: usize,
    kind: PhantomData<fn() -> T>,
}

impl<T> Id<T> {
    fn new(index: usize) -> Self {
        Self {
            index,
            kind: PhantomData,
        }
    }

    pub(crate) fn all(items: &[T]) -> impl Iterator<Item = Self> + use<T> {
        (0..items.len()).map(Self::new)
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for Id<T> {}

impl<T> fmt::Debug for Id<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Id({})", self.index)
    }
}

macro_rules! arena {
    ($($field:ident: $item:ty),* $(,)?) => {
        $(
            impl Index<Id<$item>> for Api {
                type Output = $item;

                fn index(&self, id: Id<$item>) -> &$item {
                    &self.$field[id.index]
                }
            }
        )*
    };
}

arena! {
    constants: Constant,
    structs: Struct,
    enums: Enum,
    flags: Flags,
    callbacks: Callback,
    classes: Class,
    functions: Function,
}

#[derive(Debug)]
pub(crate) struct Api {
    pub(crate) name: String,
    pub(crate) version: Version,
    pub(crate) abi_prefix: String,
    pub(crate) doc: Doc,
    pub(crate) constants: Vec<Constant>,
    pub(crate) structs: Vec<Struct>,
    pub(crate) enums: Vec<Enum>,
    pub(crate) flags: Vec<Flags>,
    pub(crate) callbacks: Vec<Callback>,
    pub(crate) classes: Vec<Class>,
    pub(crate) functions: Vec<Function>,
}

impl Api {
    pub(crate) fn member(&self, class: Id<Class>, index: usize) -> &Member {
        &self[class].members[index]
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Version {
    pub(crate) major: u32,
    pub(crate) minor: u32,
    pub(crate) patch: u32,
}

impl Version {
    pub(crate) fn packed(self) -> u32 {
        (self.major << 22) | (self.minor << 12) | self.patch
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Primitive {
    Bool,
    U8,
    U16,
    U32,
    U64,
    I32,
    I64,
    Usize,
    F32,
    F64,
}

impl Primitive {
    pub(crate) fn named(name: &str) -> Option<Self> {
        Some(match name {
            "bool" => Self::Bool,
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "usize" => Self::Usize,
            "f32" => Self::F32,
            "f64" => Self::F64,
            _ => return None,
        })
    }

    pub(crate) fn integer_range(self) -> Option<(i128, i128)> {
        Some(match self {
            Self::Bool | Self::F32 | Self::F64 => return None,
            Self::U8 => (0, u8::MAX.into()),
            Self::U16 => (0, u16::MAX.into()),
            Self::U32 => (0, u32::MAX.into()),
            Self::U64 | Self::Usize => (0, u64::MAX.into()),
            Self::I32 => (i32::MIN.into(), i32::MAX.into()),
            Self::I64 => (i64::MIN.into(), i64::MAX.into()),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ownership {
    Borrowed,
    Owned,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Value {
    Primitive(Primitive),
    Struct(Id<Struct>),
    Enum(Id<Enum>),
    Flags(Id<Flags>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Element {
    Primitive(Primitive),
    String,
}

#[derive(Clone, Debug)]
pub(crate) enum Input {
    Value(Value),
    String,
    Object {
        class: Id<Class>,
        ownership: Ownership,
    },
    Array(Element),
    Closure(Closure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Output {
    Value(Value),
    String(Ownership),
    Object {
        class: Id<Class>,
        ownership: Ownership,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Threading {
    ThisThread,
    AnyThread,
}

#[derive(Clone, Debug)]
pub(crate) struct Closure {
    pub(crate) functions: Vec<ClosureFunction>,
    pub(crate) threading: Threading,
}

#[derive(Clone, Debug)]
pub(crate) struct ClosureFunction {
    pub(crate) name: String,
    pub(crate) callback: Id<Callback>,
}

#[derive(Clone, Debug)]
pub(crate) struct Param {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Paragraph,
    pub(crate) ty: Input,
}

#[derive(Clone, Debug)]
pub(crate) struct Signature {
    pub(crate) params: Vec<Param>,
    pub(crate) returns: Option<Output>,
    pub(crate) throws: Option<Id<Enum>>,
}

#[derive(Debug)]
pub(crate) struct Constant {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) ty: Primitive,
    pub(crate) value: ConstantValue,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ConstantValue {
    Integer(i128),
    Float(f64),
    Maximum,
}

#[derive(Debug)]
pub(crate) struct Struct {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) fields: Vec<Field>,
}

#[derive(Debug)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) ty: Primitive,
}

#[derive(Debug)]
pub(crate) struct Enum {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) openness: Openness,
    pub(crate) error: Option<ErrorCodes>,
    pub(crate) value_prefix: String,
    pub(crate) variants: Vec<Variant>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Openness {
    Closed,
    Open,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ErrorCodes {
    pub(crate) success: usize,
    pub(crate) message: Id<Function>,
}

#[derive(Debug)]
pub(crate) struct Flags {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) value_prefix: String,
    pub(crate) flags: Vec<Variant>,
}

#[derive(Debug)]
pub(crate) struct Variant {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) value: u32,
}

#[derive(Debug)]
pub(crate) struct Callback {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) params: Vec<Param>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Configuration {
    Plain,
    Builder,
}

#[derive(Debug)]
pub(crate) struct Class {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) lifetime: Lifetime,
    pub(crate) configuration: Configuration,
    pub(crate) members: Vec<Member>,
}

impl Class {
    pub(crate) fn has_containers(&self) -> bool {
        self.members.iter().any(|member| {
            matches!(
                member,
                Member::Declaration(Declaration {
                    kind: DeclarationKind::Container,
                    ..
                })
            )
        })
    }
}

#[derive(Debug)]
pub(crate) enum Member {
    Constructor(Constructor),
    Method(Method),
    Property(Property),
    Declaration(Declaration),
}

impl Member {
    pub(crate) fn span(&self) -> Span {
        match self {
            Self::Constructor(member) => member.span,
            Self::Method(member) => member.span,
            Self::Property(member) => member.span,
            Self::Declaration(member) => member.span,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Constructor {
    pub(crate) name: Option<String>,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) params: Vec<Param>,
    pub(crate) thread_safe: bool,
}

#[derive(Debug)]
pub(crate) struct Method {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) receiver: Ownership,
    pub(crate) signature: Signature,
    pub(crate) thread_safe: bool,
}

#[derive(Debug)]
pub(crate) struct Property {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) ty: Output,
    pub(crate) access: Access,
    pub(crate) thread_safe: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Access {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug)]
pub(crate) struct Declaration {
    pub(crate) kind: DeclarationKind,
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) params: Vec<Param>,
}

#[derive(Debug)]
pub(crate) struct Function {
    pub(crate) name: String,
    pub(crate) span: Span,
    pub(crate) doc: Doc,
    pub(crate) signature: Signature,
    pub(crate) thread_safe: bool,
}
