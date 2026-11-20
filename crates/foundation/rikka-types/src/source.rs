use std::borrow::Cow;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub enum Source<'a> {
    File(Cow<'a, Path>),
    Bytes(Bytes<'a>),
}

#[derive(Clone, Debug)]
pub enum Bytes<'a> {
    Borrowed(&'a [u8]),
    Shared(Arc<[u8]>),
}

impl<'a> Source<'a> {
    #[must_use]
    pub fn into_owned(self) -> Source<'static> {
        match self {
            Source::File(path) => Source::File(Cow::Owned(path.into_owned())),
            Source::Bytes(bytes) => Source::Bytes(Bytes::Shared(bytes.into_shared())),
        }
    }

    #[must_use]
    pub fn borrowed(&'a self) -> Source<'a> {
        match self {
            Source::File(path) => Source::File(Cow::Borrowed(path)),
            Source::Bytes(bytes) => Source::Bytes(Bytes::Borrowed(bytes)),
        }
    }
}

impl Bytes<'_> {
    #[must_use]
    pub fn into_shared(self) -> Arc<[u8]> {
        match self {
            Bytes::Borrowed(bytes) => Arc::from(bytes),
            Bytes::Shared(bytes) => bytes,
        }
    }
}

impl Deref for Bytes<'_> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            Bytes::Borrowed(bytes) => bytes,
            Bytes::Shared(bytes) => bytes,
        }
    }
}

impl AsRef<[u8]> for Bytes<'_> {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl<'a> From<&'a Path> for Source<'a> {
    fn from(path: &'a Path) -> Source<'a> {
        Source::File(Cow::Borrowed(path))
    }
}

impl<'a> From<&'a PathBuf> for Source<'a> {
    fn from(path: &'a PathBuf) -> Source<'a> {
        Source::File(Cow::Borrowed(path))
    }
}

impl From<PathBuf> for Source<'static> {
    fn from(path: PathBuf) -> Source<'static> {
        Source::File(Cow::Owned(path))
    }
}

impl<'a> From<&'a str> for Source<'a> {
    fn from(path: &'a str) -> Source<'a> {
        Source::File(Cow::Borrowed(Path::new(path)))
    }
}

impl<'a> From<&'a String> for Source<'a> {
    fn from(path: &'a String) -> Source<'a> {
        Source::File(Cow::Borrowed(Path::new(path)))
    }
}

impl<'a> From<&'a [u8]> for Source<'a> {
    fn from(bytes: &'a [u8]) -> Source<'a> {
        Source::Bytes(Bytes::Borrowed(bytes))
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for Source<'a> {
    fn from(bytes: &'a [u8; N]) -> Source<'a> {
        Source::Bytes(Bytes::Borrowed(bytes))
    }
}

impl<'a> From<&'a Vec<u8>> for Source<'a> {
    fn from(bytes: &'a Vec<u8>) -> Source<'a> {
        Source::Bytes(Bytes::Borrowed(bytes))
    }
}

impl From<Vec<u8>> for Source<'static> {
    fn from(bytes: Vec<u8>) -> Source<'static> {
        Source::Bytes(Bytes::Shared(Arc::from(bytes)))
    }
}

impl From<Arc<[u8]>> for Source<'static> {
    fn from(bytes: Arc<[u8]>) -> Source<'static> {
        Source::Bytes(Bytes::Shared(bytes))
    }
}

impl<'a> From<&'a Arc<[u8]>> for Source<'a> {
    fn from(bytes: &'a Arc<[u8]>) -> Source<'a> {
        Source::Bytes(Bytes::Borrowed(bytes))
    }
}

impl<'a> From<Bytes<'a>> for Source<'a> {
    fn from(bytes: Bytes<'a>) -> Source<'a> {
        Source::Bytes(bytes)
    }
}
