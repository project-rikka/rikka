use std::rc::Rc;

pub use rikka_compositor_types::MimeList;

#[derive(Debug)]
pub(crate) enum MimeTypes {
    Collecting(Vec<Box<str>>),
    Frozen(MimeList),
}

impl MimeTypes {
    pub(crate) fn as_slice(&self) -> &[Box<str>] {
        match self {
            Self::Collecting(collecting) => collecting,
            Self::Frozen(frozen) => frozen,
        }
    }

    pub(crate) fn contains(&self, mime_type: &str) -> bool {
        self.as_slice()
            .iter()
            .any(|offered| &**offered == mime_type)
    }

    pub(crate) fn add(&mut self, mime_type: &str) {
        if self.contains(mime_type) {
            return;
        }
        match self {
            Self::Collecting(collecting) => collecting.push(mime_type.into()),
            Self::Frozen(frozen) => {
                *frozen = frozen
                    .iter()
                    .cloned()
                    .chain(std::iter::once(mime_type.into()))
                    .collect();
            }
        }
    }

    pub(crate) fn is_collecting(&self) -> bool {
        matches!(self, Self::Collecting(_))
    }

    pub(crate) fn freeze(&mut self) {
        if let Self::Collecting(collecting) = self {
            *self = Self::Frozen(Rc::from(std::mem::take(collecting)));
        }
    }

    pub(crate) fn frozen(&self) -> Option<&MimeList> {
        match self {
            Self::Collecting(_) => None,
            Self::Frozen(frozen) => Some(frozen),
        }
    }
}
