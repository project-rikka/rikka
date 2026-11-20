use rikka_file_format::DecodeError;

use crate::note::SkipReason;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Mesh(#[from] rikka_polymesh::Error),

    #[error("modifier \"{name}\" is disabled, skipping apply")]
    Disabled { name: String },

    #[error("modifier \"{name}\" cannot be applied to a mesh with shape keys")]
    ShapeKeys { name: String },

    #[error(
        "modifier \"{name}\" does not deform, only deforming modifiers can be applied to shapes"
    )]
    NotDeforming { name: String },

    #[error("modifier \"{name}\" was not evaluated: {reason}")]
    Skipped { name: String, reason: SkipReason },

    #[error(transparent)]
    Decode(#[from] DecodeError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
