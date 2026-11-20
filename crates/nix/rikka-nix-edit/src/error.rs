use crate::path::AttrPath;

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("Nix syntax error at line {line}, column {column}: {message}")]
    Syntax {
        line: usize,
        column: usize,
        message: String,
    },
    #[error("`{0}` is not an attribute path")]
    AttrPath(String),
    #[error("`{0}` is not a Nix identifier")]
    Identifier(String),
    #[error("`{0}` is not a Nix path")]
    Path(String),
    #[error("{0} cannot be written as a Nix number")]
    Number(String),
    #[error("document is neither an attribute set nor a function returning one")]
    NotAttrSet,
    #[error("document is not a function taking an attribute set pattern")]
    NotFunction,
    #[error("the argument pattern holds {0} and requires evaluation")]
    OpaquePattern(&'static str),
    #[error("{path} is {found} and requires evaluation")]
    Opaque { path: AttrPath, found: &'static str },
    #[error("{path} is not {expected}")]
    Mismatch {
        path: AttrPath,
        expected: &'static str,
    },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
