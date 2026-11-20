mod com;
mod compiler;
mod error;
mod exit;
mod ffi;
mod file_system;
mod layout;
mod program;
mod reflect;
mod schema;
mod spirv;
mod worker;

pub use crate::compiler::{Compiler, CompilerBuilder, Optimization};
pub use crate::error::{Error, Result};
pub use crate::layout::{
    Access, Argument, Attribute, Binding, EntryPoint, Field, Parameter, Scalar, Shape, Stage, Type,
    TypeLayout,
};
pub use crate::program::{Program, Source};
pub use crate::schema::{
    Bounds, Hint, Literal, ParameterKind, ParameterSchema, ParameterSpec, Preset, Role,
    SchemaError, TEXTURE_TYPE, TextureKind,
};
