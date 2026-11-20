use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    #[error("element {element} references vertex {index}, but there are {len} vertices")]
    IndexOutOfRange {
        element: usize,
        index: u32,
        len: usize,
    },
    #[error("{count} elements exceed the u32 index range")]
    TooManyElements { count: usize },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
