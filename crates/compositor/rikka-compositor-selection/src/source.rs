use std::os::fd::OwnedFd;

use rikka_compositor_protocols::ext_data_control_source_v1::ExtDataControlSourceV1;
use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_protocols::wl_data_source::WlDataSource;
use rikka_compositor_protocols::zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1;
use rikka_compositor_seat::SeatId;
use rikka_wayland_wire::ClientId;

use crate::access::Access;
use crate::event::{SendDrag, SendSelection};
use crate::ids::{CompositorSource, SourceId};
use crate::kind::SelectionKind;
use crate::mime::{MimeList, MimeTypes};

pub(crate) const ACTIONS_SINCE: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    Client(ClientId),
    Compositor(CompositorSource),
}

#[derive(Debug, Clone)]
pub(crate) enum Origin {
    Data(WlDataSource),
    Primary(ZwpPrimarySelectionSourceV1),
    Control(ExtDataControlSourceV1),
    Compositor { seat: SeatId, kind: SelectionKind },
    Dragged { seat: SeatId },
}

#[derive(Debug)]
pub(crate) struct SourceRecord {
    pub(crate) origin: Origin,
    pub(crate) mime_types: MimeTypes,
    pub(crate) actions: Option<DndAction>,
}

impl SourceRecord {
    pub(crate) fn new(origin: Origin) -> Self {
        Self {
            origin,
            mime_types: MimeTypes::Collecting(Vec::new()),
            actions: None,
        }
    }

    pub(crate) fn frozen(origin: Origin, mime_types: MimeList) -> Self {
        Self {
            origin,
            mime_types: MimeTypes::Frozen(mime_types),
            actions: None,
        }
    }

    pub(crate) fn public(&self, id: SourceId) -> Source {
        match &self.origin {
            Origin::Data(source) => Source::Client(source.client()),
            Origin::Primary(source) => Source::Client(source.client()),
            Origin::Control(source) => Source::Client(source.client()),
            Origin::Compositor { .. } | Origin::Dragged { .. } => {
                Source::Compositor(CompositorSource(id))
            }
        }
    }

    pub(crate) fn offers(&self, mime_type: &str) -> bool {
        self.mime_types.contains(mime_type)
    }

    pub(crate) fn offer(&mut self, mime_type: &str) {
        self.mime_types.add(mime_type);
    }

    pub(crate) fn is_fresh(&self) -> bool {
        self.mime_types.is_collecting()
    }

    pub(crate) fn freeze(&mut self) {
        self.mime_types.freeze();
    }

    pub(crate) fn is_dragged(&self) -> bool {
        matches!(self.origin, Origin::Dragged { .. })
    }

    pub(crate) fn data_source(&self) -> Option<&WlDataSource> {
        match &self.origin {
            Origin::Data(source) => Some(source),
            _ => None,
        }
    }

    pub(crate) fn drag_actions(&self) -> DndAction {
        match &self.origin {
            Origin::Data(source) if source.version() < ACTIONS_SINCE => DndAction::COPY,
            _ => self.actions.unwrap_or(DndAction::empty()),
        }
    }

    pub(crate) fn replaced(&self) {
        match &self.origin {
            Origin::Data(source) => source.cancelled(),
            Origin::Primary(source) => source.cancelled(),
            Origin::Control(source) => source.cancelled(),
            Origin::Compositor { .. } | Origin::Dragged { .. } => {}
        }
    }
}

pub(crate) fn deliver<A: Access>(state: &mut A, source: SourceId, mime_type: &str, fd: OwnedFd) {
    let Some(record) = state.selections().sources.get(source) else {
        return;
    };
    if !record.offers(mime_type) {
        return;
    }
    match record.origin.clone() {
        Origin::Data(target) => target.send(mime_type, fd),
        Origin::Primary(target) => target.send(mime_type, fd),
        Origin::Control(target) => target.send(mime_type, fd),
        Origin::Compositor { seat, kind } => state.send_selection(SendSelection {
            seat,
            kind,
            source: CompositorSource(source),
            mime_type: mime_type.to_owned(),
            fd,
        }),
        Origin::Dragged { seat } => state.send_drag(SendDrag {
            seat,
            source: CompositorSource(source),
            mime_type: mime_type.to_owned(),
            fd,
        }),
    }
}
