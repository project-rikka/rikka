use rikka_compositor_protocols::ext_data_control_device_v1::ExtDataControlDeviceV1;
use rikka_compositor_protocols::wl_data_device::WlDataDevice;
use rikka_compositor_protocols::wl_data_device_manager::DndAction;
use rikka_compositor_protocols::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1;
use rikka_compositor_seat::SeatId;
use rikka_compositor_server::ByClient;
use rikka_wayland_wire::ClientId;
use slotmap::{SecondaryMap, SlotMap};

use crate::access::Queue;
use crate::drag::{Drag, DragOffer, DragView};
use crate::ids::{CompositorSource, DragOfferId, ForeignOffer, SourceId};
use crate::kind::SelectionKind;
use crate::mime::MimeList;
use crate::source::{Source, SourceRecord};

#[derive(Debug, Default)]
pub struct Selections {
    pub(crate) seats: SecondaryMap<SeatId, SeatSelections>,
    pub(crate) drags: SecondaryMap<SeatId, Drag>,
    pub(crate) sources: SlotMap<SourceId, SourceRecord>,
    pub(crate) drag_offers: SlotMap<DragOfferId, DragOffer>,
    pub(crate) pending: Queue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Current<'a> {
    pub source: Source,
    pub mime_types: &'a MimeList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignOfferInfo<'a> {
    pub mime_types: &'a MimeList,
    pub actions: DndAction,
}

#[derive(Debug, Default)]
pub(crate) struct SeatSelections {
    pub(crate) clipboard: Slot,
    pub(crate) primary: Slot,
    pub(crate) data_devices: ByClient<WlDataDevice>,
    pub(crate) primary_devices: ByClient<ZwpPrimarySelectionDeviceV1>,
    pub(crate) control_devices: Vec<ExtDataControlDeviceV1>,
    pub(crate) focus: Vec<Focused>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Focused {
    pub(crate) client: ClientId,
    pub(crate) since: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) source: Option<SourceId>,
    pub(crate) serial: u32,
}

impl SeatSelections {
    pub(crate) fn slot(&self, kind: SelectionKind) -> Slot {
        match kind {
            SelectionKind::Clipboard => self.clipboard,
            SelectionKind::Primary => self.primary,
        }
    }

    pub(crate) fn slot_mut(&mut self, kind: SelectionKind) -> &mut Slot {
        match kind {
            SelectionKind::Clipboard => &mut self.clipboard,
            SelectionKind::Primary => &mut self.primary,
        }
    }

    pub(crate) fn focused_since(&self, client: ClientId) -> Option<u32> {
        self.focus
            .iter()
            .find(|focused| focused.client == client)
            .map(|focused| focused.since)
    }
}

impl Selections {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn current(&self, seat: SeatId, kind: SelectionKind) -> Option<Current<'_>> {
        let source = self.seats.get(seat)?.slot(kind).source?;
        let record = self.sources.get(source)?;
        Some(Current {
            source: record.public(source),
            mime_types: record.mime_types.frozen()?,
        })
    }

    pub fn drag(&self, seat: SeatId) -> Option<DragView> {
        let drag = self.drags.get(seat)?;
        Some(drag.view(&self.drag_offers))
    }

    pub fn drag_mime_types(&self, seat: SeatId) -> Option<&MimeList> {
        let source = self.drags.get(seat)?.source?;
        self.sources.get(source)?.mime_types.frozen()
    }

    pub fn foreign_offer(&self, offer: ForeignOffer) -> Option<ForeignOfferInfo<'_>> {
        let record = self.sources.get(self.drag_offers.get(offer.0)?.source)?;
        Some(ForeignOfferInfo {
            mime_types: record.mime_types.frozen()?,
            actions: record.drag_actions(),
        })
    }

    pub fn compositor_drag_offers(&self, source: CompositorSource, mime_type: &str) -> bool {
        self.sources
            .get(source.0)
            .is_some_and(|record| record.offers(mime_type))
    }

    pub(crate) fn seat(&self, seat: SeatId) -> Option<&SeatSelections> {
        self.seats.get(seat)
    }

    pub(crate) fn seat_mut(&mut self, seat: SeatId) -> &mut SeatSelections {
        if !self.seats.contains_key(seat) {
            self.seats.insert(seat, SeatSelections::default());
        }
        &mut self.seats[seat]
    }
}
