mod composite;
mod error;
mod material;

use rikka_shader_graph::Library;

pub use crate::composite::{Resolve, builtin_of, composite, filters, pixel_effect};
pub use crate::error::Error;
pub use crate::material::{Material, socket_value};

pub fn library() -> Result<Library, Error> {
    let mut library = Library::standard();
    library.extend(filters()?);
    Ok(library)
}
