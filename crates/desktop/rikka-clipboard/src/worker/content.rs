use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;

use rikka_clipboard_core::{Error, Offer, Result};

use super::state::{pipe_with_data, write_transfer};

pub(crate) type Delivery = Box<dyn Fn(&str, OwnedFd) + Send>;

pub(crate) enum Content {
    Data(Offer),
    Deferred {
        mime_types: Vec<String>,
        deliver: Delivery,
    },
}

impl Default for Content {
    fn default() -> Self {
        Content::Data(Offer::new())
    }
}

impl Content {
    pub(super) fn is_empty(&self) -> bool {
        match self {
            Content::Data(offer) => offer.is_empty(),
            Content::Deferred { mime_types, .. } => mime_types.is_empty(),
        }
    }

    pub(super) fn mime_types(&self) -> Vec<String> {
        match self {
            Content::Data(offer) => offer.mime_types().map(str::to_owned).collect(),
            Content::Deferred { mime_types, .. } => mime_types.clone(),
        }
    }

    fn offers(&self, mime: &str) -> bool {
        match self {
            Content::Data(offer) => offer.mime_types().any(|known| known == mime),
            Content::Deferred { mime_types, .. } => mime_types.iter().any(|known| known == mime),
        }
    }

    pub(super) fn send(&self, mime: &str, fd: OwnedFd) {
        match self {
            Content::Data(offer) => {
                if let Some(data) = offer.data(mime) {
                    write_transfer(data, fd);
                }
            }
            Content::Deferred { deliver, .. } => {
                if self.offers(mime) {
                    deliver(mime, fd);
                }
            }
        }
    }

    pub(super) fn open(&self, mime: &str) -> Option<Result<UnixStream>> {
        match self {
            Content::Data(offer) => offer.data(mime).map(pipe_with_data),
            Content::Deferred { deliver, .. } => self.offers(mime).then(|| {
                UnixStream::pair().map_err(Error::Io).map(|(ours, theirs)| {
                    deliver(mime, OwnedFd::from(theirs));
                    ours
                })
            }),
        }
    }
}
