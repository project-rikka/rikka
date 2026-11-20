use rikka_compositor_protocols::wp_drm_lease_connector_v1::WpDrmLeaseConnectorV1;
use rikka_compositor_protocols::wp_drm_lease_device_v1::WpDrmLeaseDeviceV1;
use rikka_compositor_protocols::wp_drm_lease_request_v1::WpDrmLeaseRequestV1;
use rikka_compositor_protocols::wp_drm_lease_v1::WpDrmLeaseV1;
use rikka_compositor_server::{DisplayHandle, Module};

use super::host::DrmLeaseHost;
use super::protocol;

#[derive(Debug, Clone, Copy, Default)]
pub struct DrmLeaseProtocols;

impl<S: DrmLeaseHost> Module<S> for DrmLeaseProtocols {
    fn install(self, display: &DisplayHandle<S>) {
        display
            .serve::<WpDrmLeaseDeviceV1>(protocol::device)
            .bound(protocol::bound)
            .destroyed(protocol::device_destroyed);
        display
            .serve::<WpDrmLeaseConnectorV1>(protocol::connector)
            .destroyed(protocol::connector_destroyed);
        display
            .serve::<WpDrmLeaseRequestV1>(protocol::request)
            .destroyed(protocol::request_destroyed);
        display
            .serve::<WpDrmLeaseV1>(protocol::lease)
            .destroyed(protocol::lease_destroyed);
    }
}
