use rikka_compositor_protocols::ext_data_control_device_v1::ExtDataControlDeviceV1;
use rikka_compositor_protocols::ext_data_control_offer_v1::ExtDataControlOfferV1;
use rikka_compositor_protocols::wl_data_device::WlDataDevice;
use rikka_compositor_protocols::wl_data_offer::WlDataOffer;
use rikka_compositor_protocols::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1;
use rikka_compositor_protocols::zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1;
use rikka_compositor_server::Client;
use rikka_wayland_wire::Resource;

use crate::data_device::OfferTarget;
use crate::ids::SourceId;
use crate::kind::SelectionKind;
use crate::mime::MimeList;

#[derive(Debug, Clone)]
pub(crate) struct Snapshot {
    pub(crate) source: SourceId,
    pub(crate) mime_types: MimeList,
}

pub(crate) fn to_data_device(device: &WlDataDevice, selection: Option<&Snapshot>) {
    let offer = selection.and_then(|selection| {
        let offer: WlDataOffer = create(device)?;
        OfferTarget::Selection(selection.source).attach(&offer);
        device.data_offer(&offer);
        for mime_type in selection.mime_types.iter() {
            offer.offer(mime_type);
        }
        Some(offer)
    });
    device.selection(offer.as_ref());
}

pub(crate) fn to_primary_device(
    device: &ZwpPrimarySelectionDeviceV1,
    selection: Option<&Snapshot>,
) {
    let offer = selection.and_then(|selection| {
        let offer: ZwpPrimarySelectionOfferV1 = create(device)?;
        offer.set_key(selection.source);
        device.data_offer(&offer);
        for mime_type in selection.mime_types.iter() {
            offer.offer(mime_type);
        }
        Some(offer)
    });
    device.selection(offer.as_ref());
}

pub(crate) fn to_control_device(
    device: &ExtDataControlDeviceV1,
    kind: SelectionKind,
    selection: Option<&Snapshot>,
) {
    let offer = selection.and_then(|selection| {
        let offer: ExtDataControlOfferV1 = create(device)?;
        offer.set_key(selection.source);
        device.data_offer(&offer);
        for mime_type in selection.mime_types.iter() {
            offer.offer(mime_type);
        }
        Some(offer)
    });
    match kind {
        SelectionKind::Clipboard => device.selection(offer.as_ref()),
        SelectionKind::Primary => device.primary_selection(offer.as_ref()),
    }
}

pub(crate) fn create<R: Resource>(device: &impl Resource) -> Option<R> {
    let client = Client::from_resource(device)?;
    Some(client.create::<R>(device.object().version()))
}
