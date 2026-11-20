use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;

use rikka_wayland::Display;
use rikka_wayland_client::Route;
use rikka_wayland_client::ext_data_control_device_v1::{self, ExtDataControlDeviceV1};
use rikka_wayland_client::ext_data_control_manager_v1::ExtDataControlManagerV1;
use rikka_wayland_client::ext_data_control_offer_v1::{self, ExtDataControlOfferV1};
use rikka_wayland_client::ext_data_control_source_v1::{self, ExtDataControlSourceV1};
use rikka_wayland_client::wl_seat::WlSeat;

use super::content::Content;
use super::state::{Published, State, ensure_offered, transfer};
use rikka_clipboard_core::{Error, Result, Selection};

#[derive(Default)]
struct Side {
    published: Published<ExtDataControlSourceV1>,
    offer: Option<ExtDataControlOfferV1>,
    mime_types: Vec<String>,
}

impl Side {
    fn settle(&mut self, id: Option<ExtDataControlOfferV1>, mime_types: Vec<String>) {
        if let Some(old) = self.offer.take() {
            old.destroy();
        }
        self.mime_types = mime_types;
        self.offer = id;
    }
}

pub(super) struct Control {
    manager: ExtDataControlManagerV1,
    device: ExtDataControlDeviceV1,
    clipboard: Side,
    primary: Side,
    pending: Vec<(ExtDataControlOfferV1, Vec<String>)>,
}

impl Control {
    pub(super) fn bind(display: &Display, route: &Route, seat: &WlSeat) -> Option<Control> {
        let manager: ExtDataControlManagerV1 = display.bind_on(route, 1..=1).ok()?;
        let device = manager.get_data_device(seat);
        Some(Control {
            manager,
            device,
            clipboard: Side::default(),
            primary: Side::default(),
            pending: Vec::new(),
        })
    }

    fn side(&self, selection: Selection) -> &Side {
        match selection {
            Selection::Primary => &self.primary,
            Selection::Clipboard => &self.clipboard,
        }
    }

    fn side_mut(&mut self, selection: Selection) -> &mut Side {
        match selection {
            Selection::Primary => &mut self.primary,
            Selection::Clipboard => &mut self.clipboard,
        }
    }

    fn sides_mut(&mut self) -> [&mut Side; 2] {
        [&mut self.clipboard, &mut self.primary]
    }

    pub(super) fn owner(&self, selection: Selection) -> Option<u64> {
        self.side(selection).published.owner()
    }

    pub(super) fn mime_types(&self, selection: Selection) -> Vec<String> {
        let side = self.side(selection);
        side.published
            .mime_types_offered()
            .unwrap_or_else(|| side.mime_types.clone())
    }

    pub(super) fn publish(&mut self, content: Content, selection: Selection, by: u64) {
        let source = (!content.is_empty()).then(|| {
            let source = self.manager.create_data_source();
            for mime in content.mime_types() {
                source.offer(&mime);
            }
            source
        });
        match selection {
            Selection::Primary => self.device.set_primary_selection(source.as_ref()),
            Selection::Clipboard => self.device.set_selection(source.as_ref()),
        }
        let side = self.side_mut(selection);
        if let Some(old) = std::mem::replace(&mut side.published.source, source) {
            old.destroy();
        }
        side.published.content = content;
        side.published.by = by;
    }

    pub(super) fn receive(&self, mime: &str, selection: Selection) -> Result<UnixStream> {
        let side = self.side(selection);
        if let Some(stream) = side.published.open(mime) {
            return stream;
        }
        ensure_offered(&side.mime_types, mime)?;
        let offer = side.offer.as_ref().ok_or(Error::Empty)?;
        transfer(|fd| offer.receive(mime, fd))
    }

    fn send(&self, source: &ExtDataControlSourceV1, mime: &str, fd: OwnedFd) {
        if let Some(side) = [&self.clipboard, &self.primary]
            .into_iter()
            .find(|side| side.published.source.as_ref() == Some(source))
        {
            side.published.content.send(mime, fd);
        }
    }

    fn withdraw(&mut self, source: &ExtDataControlSourceV1) {
        for side in self.sides_mut() {
            if side.published.source.as_ref() == Some(source) {
                side.published = Published::default();
            }
        }
    }

    fn take(
        &mut self,
        selection: Selection,
        offer: Option<ExtDataControlOfferV1>,
    ) -> (Option<u64>, Vec<String>) {
        let types = self.claim(offer.as_ref());
        let side = self.side_mut(selection);
        let owner = side.published.owner();
        side.settle(offer, types.clone());
        (owner, types)
    }

    fn claim(&mut self, offer: Option<&ExtDataControlOfferV1>) -> Vec<String> {
        let Some(offer) = offer else {
            return Vec::new();
        };
        let at = self.pending.iter().position(|(known, _)| known == offer);
        at.map(|at| self.pending.remove(at).1).unwrap_or_default()
    }
}

impl Drop for Control {
    fn drop(&mut self) {
        self.device.destroy();
        self.manager.destroy();
    }
}

pub(super) fn on_ext_data_control_device_v1(
    state: &mut State,
    _proxy: ExtDataControlDeviceV1,
    event: ext_data_control_device_v1::Event,
) {
    let Some(control) = state.control.as_mut() else {
        return;
    };
    match event {
        ext_data_control_device_v1::Event::DataOffer { id } => {
            control.pending.push((id, Vec::new()));
        }
        ext_data_control_device_v1::Event::Selection { id } => {
            let (owner, types) = control.take(Selection::Clipboard, id);
            state.announce(Selection::Clipboard, owner, types);
        }
        ext_data_control_device_v1::Event::PrimarySelection { id } => {
            let (owner, types) = control.take(Selection::Primary, id);
            state.announce(Selection::Primary, owner, types);
        }
        ext_data_control_device_v1::Event::Finished => state.control = None,
        _ => {}
    }
}

pub(super) fn on_ext_data_control_offer_v1(
    state: &mut State,
    proxy: ExtDataControlOfferV1,
    event: ext_data_control_offer_v1::Event,
) {
    let ext_data_control_offer_v1::Event::Offer { mime_type } = event else {
        return;
    };
    let Some(control) = state.control.as_mut() else {
        return;
    };
    if let Some((_, types)) = control
        .pending
        .iter_mut()
        .find(|(known, _)| *known == proxy)
    {
        types.push(mime_type);
    }
}

pub(super) fn on_ext_data_control_source_v1(
    state: &mut State,
    proxy: ExtDataControlSourceV1,
    event: ext_data_control_source_v1::Event,
) {
    let Some(control) = state.control.as_mut() else {
        return;
    };
    match event {
        ext_data_control_source_v1::Event::Send { mime_type, fd } => {
            control.send(&proxy, &mime_type, fd);
        }
        ext_data_control_source_v1::Event::Cancelled => {
            control.withdraw(&proxy);
            proxy.destroy();
        }
        _ => {}
    }
}
