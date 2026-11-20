use rikka_ui::{Accept, Delivery, Offer};

use crate::wayland::data_device::{self, Answer};

pub(super) struct Wire(pub(super) data_device::Delivery);

impl Delivery for Wire {
    fn read(&self, mime: &str) -> std::io::Result<Vec<u8>> {
        self.0.read(mime).map_err(std::io::Error::other)
    }
}

pub(super) struct Local {
    pub(super) offer: Offer,
    pub(super) _wire: data_device::Delivery,
}

impl Delivery for Local {
    fn read(&self, mime: &str) -> std::io::Result<Vec<u8>> {
        self.offer
            .data(mime)
            .map(|data| data.to_vec())
            .ok_or_else(|| std::io::Error::other(format!("drag does not offer `{mime}`")))
    }
}

pub(super) fn answer(accept: &Accept) -> Answer {
    Answer {
        mime: accept.mime.clone(),
        actions: accept.actions,
        preferred: accept.preferred,
    }
}
