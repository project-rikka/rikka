use rikka_compositor_protocols::wl_data_offer::{Error, Request, WlDataOffer};

use crate::access::Access;
use crate::data_device::OfferTarget;
use crate::drag;
use crate::source;

pub(crate) fn offer_request<A: Access>(state: &mut A, offer: WlDataOffer, request: Request) {
    match OfferTarget::from_offer(&offer) {
        Some(OfferTarget::Selection(source)) => match request {
            Request::Receive { mime_type, fd } => source::deliver(state, source, mime_type, fd),
            Request::Finish => offer.post_error(Error::InvalidFinish),
            Request::SetActions { .. } => offer.post_error(Error::InvalidOffer),
            Request::Accept { .. } | Request::Destroy => {}
        },
        Some(OfferTarget::Drag(id)) => drag::offer_request(state, &offer, id, request),
        None => {}
    }
}

pub(crate) fn offer_destroyed<A: Access>(state: &mut A, offer: WlDataOffer) {
    if let Some(OfferTarget::Drag(id)) = OfferTarget::from_offer(&offer) {
        drag::offer_destroyed(state, &offer, id);
    }
}
