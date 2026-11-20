mod error;
mod instruction;
mod invariant;
mod reflect;
mod shader;
mod slang;
mod words;

pub use crate::error::{Error, Result};
pub use crate::instruction::{Instruction, Instructions, instructions, literal_string};
pub use crate::invariant::invariant_position;
pub use crate::reflect::{Binding, Descriptor, Model, Reflection, Scalar, Slots, reflect};
pub use crate::shader::{ENTRY_POINT, Facts, Shader, content_hash};
pub use crate::slang::{SLANG_PROFILE, SLANG_QUIET_WARNINGS, SLANG_WARNINGS_AS_ERRORS};
pub use crate::words::{bytes, words};
