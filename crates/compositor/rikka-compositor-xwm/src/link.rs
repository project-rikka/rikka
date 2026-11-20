use std::borrow::Cow;

use x11rb_protocol::protocol::xproto::{
    Atom, AtomEnum, CreateWindowAux, CreateWindowRequest, EventMask, SELECTION_NOTIFY_EVENT,
    SelectionNotifyEvent, SendEventRequest, Timestamp, Window, WindowClass,
};

use crate::atoms::Atoms;
use crate::fault::Outcome;
use crate::wire::{COPY_DEPTH_FROM_PARENT, COPY_FROM_PARENT, NONE, Wire};

const TRANSFER_LIMIT: usize = 1024 * 1024;
const REQUEST_OVERHEAD: usize = 64;

#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectionReply {
    pub(crate) requestor: Window,
    pub(crate) selection: Atom,
    pub(crate) target: Atom,
    pub(crate) property: Atom,
    pub(crate) time: Timestamp,
}

#[derive(Debug)]
pub(crate) struct Link {
    pub(crate) wire: Wire,
    pub(crate) atoms: Atoms,
    pub(crate) root: Window,
    client_mask: u32,
    own_base: u32,
}

impl Link {
    pub(crate) fn new(wire: Wire, atoms: Atoms, root: Window, base: u32, mask: u32) -> Self {
        Self {
            wire,
            atoms,
            root,
            client_mask: mask,
            own_base: base & !mask,
        }
    }

    pub(crate) fn client_mask(&self) -> u32 {
        self.client_mask
    }

    pub(crate) fn is_ours(&self, window: Window) -> bool {
        window & !self.client_mask == self.own_base
    }

    pub(crate) fn transfer_chunk(&self) -> usize {
        (self.wire.maximum_request_bytes() - REQUEST_OVERHEAD).min(TRANSFER_LIMIT) & !3
    }

    pub(crate) fn helper_window(&mut self, events: EventMask) -> Outcome<Window> {
        let window = self.wire.generate_id()?;
        self.wire.send(CreateWindowRequest {
            depth: COPY_DEPTH_FROM_PARENT,
            wid: window,
            parent: self.root,
            x: -100,
            y: -100,
            width: 1,
            height: 1,
            border_width: 0,
            class: WindowClass::INPUT_ONLY,
            visual: COPY_FROM_PARENT,
            value_list: Cow::Owned(CreateWindowAux::new().event_mask(events)),
        })?;
        Ok(window)
    }

    pub(crate) fn set_atoms(&mut self, window: Window, property: Atom, values: &[Atom]) -> Outcome {
        self.set_words(window, property, AtomEnum::ATOM.into(), values)
    }

    pub(crate) fn set_windows(
        &mut self,
        window: Window,
        property: Atom,
        values: &[Window],
    ) -> Outcome {
        self.set_words(window, property, AtomEnum::WINDOW.into(), values)
    }

    pub(crate) fn set_cardinals(
        &mut self,
        window: Window,
        property: Atom,
        values: &[u32],
    ) -> Outcome {
        self.set_words(window, property, AtomEnum::CARDINAL.into(), values)
    }

    pub(crate) fn set_words(
        &mut self,
        window: Window,
        property: Atom,
        type_: Atom,
        values: &[u32],
    ) -> Outcome {
        let bytes: Vec<u8> = values
            .iter()
            .flat_map(|value| value.to_ne_bytes())
            .collect();
        self.wire
            .change_property(window, property, type_, 32, &bytes)
    }

    pub(crate) fn set_bytes(
        &mut self,
        window: Window,
        property: Atom,
        type_: Atom,
        bytes: &[u8],
    ) -> Outcome {
        self.wire.change_property(window, property, type_, 8, bytes)
    }

    pub(crate) fn send_event(
        &mut self,
        destination: Window,
        mask: EventMask,
        event: impl Into<[u8; 32]>,
    ) -> Outcome {
        self.wire.send(SendEventRequest {
            propagate: false,
            destination,
            event_mask: mask,
            event: Cow::Owned(event.into()),
        })?;
        Ok(())
    }

    pub(crate) fn reply_selection(&mut self, reply: SelectionReply, delivered: bool) -> Outcome {
        let property = if delivered { reply.property } else { NONE };
        self.send_event(
            reply.requestor,
            EventMask::NO_EVENT,
            SelectionNotifyEvent {
                response_type: SELECTION_NOTIFY_EVENT,
                sequence: 0,
                time: reply.time,
                requestor: reply.requestor,
                selection: reply.selection,
                target: reply.target,
                property,
            },
        )
    }
}
