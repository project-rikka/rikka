use std::ops::Deref;
use std::sync::Arc;

pub(crate) enum Source<'a> {
    Borrowed(&'a [u8]),
    Shared(Arc<[u8]>),
    Unpacked(Vec<u8>),
}

impl Deref for Source<'_> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Self::Borrowed(bytes) => bytes,
            Self::Shared(bytes) => bytes,
            Self::Unpacked(bytes) => bytes,
        }
    }
}
