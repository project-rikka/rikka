use rikka_compositor_protocols::wp_drm_lease_connector_v1::{self, WpDrmLeaseConnectorV1};
use rikka_compositor_protocols::wp_drm_lease_device_v1::{self, WpDrmLeaseDeviceV1};
use rikka_compositor_protocols::wp_drm_lease_request_v1::{self, WpDrmLeaseRequestV1};
use rikka_compositor_protocols::wp_drm_lease_v1::{self, WpDrmLeaseV1};
use rikka_compositor_server::Client;

use super::device::ConnectorRef;
use super::host::DrmLeaseHost;
use super::store::Lease;
use crate::ids::{LeaseDeviceId, LeaseId, LeaseRequestId};

pub(super) struct Pending {
    device: Option<LeaseDeviceId>,
    connectors: Vec<u32>,
}

pub(super) fn bound<S: DrmLeaseHost>(state: &mut S, resource: WpDrmLeaseDeviceV1) {
    let Some(id) = resource.key::<LeaseDeviceId>() else {
        return;
    };
    if let Some(device) = state.drm_leases().devices.get_mut(id) {
        device.bind(resource);
    }
}

pub(super) fn device<S: DrmLeaseHost>(
    state: &mut S,
    resource: WpDrmLeaseDeviceV1,
    request: wp_drm_lease_device_v1::Request,
) {
    match request {
        wp_drm_lease_device_v1::Request::CreateLeaseRequest { id } => {
            let pending = state.drm_leases().requests.insert(Pending {
                device: resource.key::<LeaseDeviceId>(),
                connectors: Vec::new(),
            });
            id.set_key(pending);
        }
        wp_drm_lease_device_v1::Request::Release => {
            forget_device(state, &resource);
            resource.released();
            if let Some(client) = Client::from_resource(&resource) {
                client.destroy(&resource);
            }
        }
    }
}

pub(super) fn device_destroyed<S: DrmLeaseHost>(state: &mut S, resource: WpDrmLeaseDeviceV1) {
    forget_device(state, &resource);
}

fn forget_device<S: DrmLeaseHost>(state: &mut S, resource: &WpDrmLeaseDeviceV1) {
    let Some(id) = resource.key::<LeaseDeviceId>() else {
        return;
    };
    if let Some(device) = state.drm_leases().devices.get_mut(id) {
        device.bound.retain(|bound| bound != resource);
    }
}

pub(super) fn connector<S>(
    _state: &mut S,
    _resource: WpDrmLeaseConnectorV1,
    request: wp_drm_lease_connector_v1::Request,
) {
    match request {
        wp_drm_lease_connector_v1::Request::Destroy => {}
    }
}

pub(super) fn connector_destroyed<S: DrmLeaseHost>(state: &mut S, resource: WpDrmLeaseConnectorV1) {
    let Some(reference) = ConnectorRef::from_instance(&resource) else {
        return;
    };
    if let Some(offer) = state
        .drm_leases()
        .devices
        .get_mut(reference.device)
        .and_then(|device| device.offer_mut(reference.connector))
    {
        offer.instances.retain(|instance| instance != &resource);
    }
}

pub(super) fn request<S: DrmLeaseHost>(
    state: &mut S,
    resource: WpDrmLeaseRequestV1,
    request: wp_drm_lease_request_v1::Request,
) {
    let Some(key) = resource.key::<LeaseRequestId>() else {
        return;
    };
    match request {
        wp_drm_lease_request_v1::Request::RequestConnector { connector } => {
            let Some(reference) = ConnectorRef::from_instance(&connector) else {
                return;
            };
            let Some(pending) = state.drm_leases().requests.get_mut(key) else {
                return;
            };
            if pending.device != Some(reference.device) {
                resource.post_error(wp_drm_lease_request_v1::Error::WrongDevice);
            } else if pending.connectors.contains(&reference.connector) {
                resource.post_error(wp_drm_lease_request_v1::Error::DuplicateConnector);
            } else {
                pending.connectors.push(reference.connector);
            }
        }
        wp_drm_lease_request_v1::Request::Submit { id } => {
            let Some(pending) = state.drm_leases().requests.remove(key) else {
                return;
            };
            if pending.connectors.is_empty() {
                resource.post_error(wp_drm_lease_request_v1::Error::EmptyLease);
                return;
            }
            submit(state, id, pending);
        }
    }
}

pub(super) fn request_destroyed<S: DrmLeaseHost>(state: &mut S, resource: WpDrmLeaseRequestV1) {
    if let Some(key) = resource.key::<LeaseRequestId>() {
        state.drm_leases().requests.remove(key);
    }
}

fn submit<S: DrmLeaseHost>(state: &mut S, lease: WpDrmLeaseV1, pending: Pending) {
    let leases = state.drm_leases();
    let available = pending
        .device
        .and_then(|device| leases.devices.get_mut(device))
        .is_some_and(|device| {
            pending.connectors.iter().all(|connector| {
                device
                    .offer_mut(*connector)
                    .is_some_and(|offer| offer.leased.is_none())
            })
        });
    let Some(device) = pending.device.filter(|_| available) else {
        lease.finished();
        return;
    };
    let grant = match state.grant_lease(device, &pending.connectors) {
        Ok(grant) => grant,
        Err(error) => {
            tracing::info!(connectors = ?pending.connectors, "DRM lease rejected: {error}");
            lease.finished();
            return;
        }
    };
    lease.lease_fd(grant.fd);
    let leases = state.drm_leases();
    let id = leases.leases.insert(Lease {
        device,
        resource: lease.clone(),
        lessee: grant.lessee,
        connectors: pending.connectors.clone(),
    });
    lease.set_key(id);
    if let Some(entry) = leases.devices.get_mut(device) {
        for connector in &pending.connectors {
            if let Some(offer) = entry.offer_mut(*connector) {
                offer.leased = Some(id);
            }
        }
        entry.retract(&pending.connectors);
    }
}

pub(super) fn lease<S>(_state: &mut S, _resource: WpDrmLeaseV1, request: wp_drm_lease_v1::Request) {
    match request {
        wp_drm_lease_v1::Request::Destroy => {}
    }
}

pub(super) fn lease_destroyed<S: DrmLeaseHost>(state: &mut S, resource: WpDrmLeaseV1) {
    let Some(id) = resource.key::<LeaseId>() else {
        return;
    };
    let leases = state.drm_leases();
    let Some(lease) = leases.leases.remove(id) else {
        return;
    };
    leases.release(&lease);
    state.revoke_lease(lease.device, lease.lessee);
}
