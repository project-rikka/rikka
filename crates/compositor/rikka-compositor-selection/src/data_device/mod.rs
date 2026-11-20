mod device;
mod manager;
mod offer;
mod source;

use rikka_compositor_protocols::wl_data_offer::WlDataOffer;

use crate::ids::{DragOfferId, SourceId};

pub(crate) use device::{device_destroyed, device_request};
pub(crate) use manager::manager_request;
pub(crate) use offer::{offer_destroyed, offer_request};
pub(crate) use source::{source_destroyed, source_request};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OfferTarget {
    Selection(SourceId),
    Drag(DragOfferId),
}

impl OfferTarget {
    pub(crate) fn from_offer(offer: &WlDataOffer) -> Option<Self> {
        offer
            .key::<SourceId>()
            .map(Self::Selection)
            .or_else(|| offer.key::<DragOfferId>().map(Self::Drag))
    }

    pub(crate) fn attach(self, offer: &WlDataOffer) {
        match self {
            Self::Selection(source) => offer.set_key(source),
            Self::Drag(drag) => offer.set_key(drag),
        }
    }
}
