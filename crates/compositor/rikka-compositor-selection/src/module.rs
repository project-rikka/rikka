use rikka_compositor_protocols::ext_data_control_device_v1::ExtDataControlDeviceV1;
use rikka_compositor_protocols::ext_data_control_manager_v1::ExtDataControlManagerV1;
use rikka_compositor_protocols::ext_data_control_offer_v1::ExtDataControlOfferV1;
use rikka_compositor_protocols::ext_data_control_source_v1::ExtDataControlSourceV1;
use rikka_compositor_protocols::wl_data_device::WlDataDevice;
use rikka_compositor_protocols::wl_data_device_manager::WlDataDeviceManager;
use rikka_compositor_protocols::wl_data_offer::WlDataOffer;
use rikka_compositor_protocols::wl_data_source::WlDataSource;
use rikka_compositor_protocols::zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1;
use rikka_compositor_protocols::zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1;
use rikka_compositor_protocols::zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1;
use rikka_compositor_protocols::zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1;
use rikka_compositor_server::{DisplayHandle, Module};

use crate::host::SelectionHost;
use crate::{control, data_device, primary};

#[derive(Debug, Clone, Copy, Default)]
pub struct SelectionProtocols;

impl<S: SelectionHost> Module<S> for SelectionProtocols {
    fn install(self, display: &DisplayHandle<S>) {
        serve_data_device(display);
        serve_primary(display);
        serve_control(display);
        display.global::<WlDataDeviceManager>().publish();
        display
            .global::<ZwpPrimarySelectionDeviceManagerV1>()
            .publish();
        display
            .global::<ExtDataControlManagerV1>()
            .privileged()
            .publish();
    }
}

fn serve_data_device<S: SelectionHost>(display: &DisplayHandle<S>) {
    display.serve::<WlDataDeviceManager>(data_device::manager_request);
    display
        .serve::<WlDataDevice>(data_device::device_request)
        .destroyed(data_device::device_destroyed);
    display
        .serve::<WlDataSource>(data_device::source_request)
        .destroyed(data_device::source_destroyed);
    display
        .serve::<WlDataOffer>(data_device::offer_request)
        .destroyed(data_device::offer_destroyed);
}

fn serve_primary<S: SelectionHost>(display: &DisplayHandle<S>) {
    display.serve::<ZwpPrimarySelectionDeviceManagerV1>(primary::manager_request);
    display
        .serve::<ZwpPrimarySelectionDeviceV1>(primary::device_request)
        .destroyed(primary::device_destroyed);
    display
        .serve::<ZwpPrimarySelectionSourceV1>(primary::source_request)
        .destroyed(primary::source_destroyed);
    display.serve::<ZwpPrimarySelectionOfferV1>(primary::offer_request);
}

fn serve_control<S: SelectionHost>(display: &DisplayHandle<S>) {
    display.serve::<ExtDataControlManagerV1>(control::manager_request);
    display
        .serve::<ExtDataControlDeviceV1>(control::device_request)
        .destroyed(control::device_destroyed);
    display
        .serve::<ExtDataControlSourceV1>(control::source_request)
        .destroyed(control::source_destroyed);
    display.serve::<ExtDataControlOfferV1>(control::offer_request);
}
