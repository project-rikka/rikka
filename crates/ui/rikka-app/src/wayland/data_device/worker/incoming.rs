use std::os::fd::AsFd as _;
use std::os::unix::net::UnixStream;

use rikka_types::DragAction;
use rikka_wayland_client::wl_data_device_manager::DndAction;
use rikka_wayland_client::wl_data_offer::WlDataOffer;

use crate::wayland::data_device::actions::{wire, wire_all};
use crate::wayland::data_device::drags::Answer;
use crate::wayland::data_device::error::{Error, Result};

pub(super) struct Incoming {
    pub(super) id: u64,
    pub(super) offer: WlDataOffer,
    serial: u32,
    accepted: Option<Answer>,
    pub(super) chosen: Option<DragAction>,
}

impl Incoming {
    pub(super) fn new(id: u64, offer: WlDataOffer, serial: u32) -> Self {
        Self {
            id,
            offer,
            serial,
            accepted: None,
            chosen: None,
        }
    }

    pub(super) fn answer(&mut self, answer: Option<Answer>) {
        if self.accepted == answer {
            return;
        }
        self.offer.accept(
            self.serial,
            answer.as_ref().map(|answer| answer.mime.as_str()),
        );
        let (actions, preferred) = match &answer {
            Some(answer) => (wire_all(answer.actions), wire(answer.preferred)),
            None => (DndAction::empty(), DndAction::empty()),
        };
        self.offer.set_actions(actions, preferred);
        self.accepted = answer;
    }

    pub(super) fn receive(&self, mime: &str) -> Result<UnixStream> {
        let (ours, theirs) = UnixStream::pair()?;
        self.offer.receive(mime, theirs.as_fd());
        drop(theirs);
        ours.shutdown(std::net::Shutdown::Write)?;
        Ok(ours)
    }

    pub(super) fn finish(self) {
        let resolved = matches!(self.chosen, Some(DragAction::Copy | DragAction::Move));
        if self.accepted.is_some() && resolved {
            self.offer.finish();
        }
        self.offer.destroy();
    }

    pub(super) fn forget(self) {
        self.offer.destroy();
    }
}

pub(super) fn unanswered() -> Error {
    Error::Unavailable("that drop is no longer offered")
}
