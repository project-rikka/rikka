mod element;
mod entities;
mod error;
mod namespaces;
mod parser;

pub use element::{Content, Element};
pub use error::{Error, Markup, Position, Result};
pub use parser::{MAX_DEPTH, parse, parse_lenient};
