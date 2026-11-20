use std::os::fd::{AsFd, OwnedFd};

use rikka_compositor_protocols::wp_drm_lease_connector_v1::WpDrmLeaseConnectorV1;
use rikka_compositor_protocols::wp_drm_lease_device_v1::WpDrmLeaseDeviceV1;
use rikka_compositor_server::{Client, Global};
use rikka_wayland_wire::ClientId;

use super::types::LeaseConnector;
use crate::ids::{LeaseDeviceId, LeaseId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ConnectorRef {
    pub(super) device: LeaseDeviceId,
    pub(super) connector: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConnectorNumber(u32);

impl rikka_wayland_wire::Key for ConnectorNumber {
    fn into_raw(self) -> u64 {
        u64::from(self.0)
    }

    fn from_raw(raw: u64) -> Self {
        Self(raw as u32)
    }
}

impl ConnectorRef {
    pub(super) fn from_instance(instance: &WpDrmLeaseConnectorV1) -> Option<Self> {
        Some(Self {
            device: instance.key::<LeaseDeviceId>()?,
            connector: instance.key::<ConnectorNumber>()?.0,
        })
    }

    fn attach(self, instance: &WpDrmLeaseConnectorV1) {
        instance.set_key(self.device);
        instance.set_key(ConnectorNumber(self.connector));
    }
}

pub(super) struct Offer {
    pub(super) connector: LeaseConnector,
    pub(super) leased: Option<LeaseId>,
    pub(super) instances: Vec<WpDrmLeaseConnectorV1>,
}

pub(super) struct Device {
    pub(super) id: LeaseDeviceId,
    pub(super) fd: OwnedFd,
    pub(super) offers: Vec<Offer>,
    pub(super) bound: Vec<WpDrmLeaseDeviceV1>,
    pub(super) global: Global,
}

impl Device {
    pub(super) fn bind(&mut self, resource: WpDrmLeaseDeviceV1) {
        resource.drm_fd(self.fd.as_fd());
        let id = self.id;
        for offer in self
            .offers
            .iter_mut()
            .filter(|offer| offer.leased.is_none())
        {
            advertise(id, offer, &resource);
        }
        resource.done();
        self.bound.push(resource);
    }

    pub(super) fn readvertise(&mut self, connectors: &[u32]) {
        let id = self.id;
        for resource in self.bound.iter().filter(|resource| resource.alive()) {
            let mut sent = false;
            for offer in self
                .offers
                .iter_mut()
                .filter(|offer| offer.leased.is_none() && connectors.contains(&offer.connector.id))
            {
                sent |= advertise(id, offer, resource);
            }
            if sent {
                resource.done();
            }
        }
    }

    pub(super) fn retract(&mut self, connectors: &[u32]) {
        let mut clients: Vec<ClientId> = Vec::new();
        for offer in self
            .offers
            .iter_mut()
            .filter(|offer| connectors.contains(&offer.connector.id))
        {
            for instance in offer
                .instances
                .drain(..)
                .filter(|instance| instance.alive())
            {
                instance.withdrawn();
                if !clients.contains(&instance.client()) {
                    clients.push(instance.client());
                }
            }
        }
        for resource in self
            .bound
            .iter()
            .filter(|resource| resource.alive() && clients.contains(&resource.client()))
        {
            resource.done();
        }
    }

    pub(super) fn offer_mut(&mut self, connector: u32) -> Option<&mut Offer> {
        self.offers
            .iter_mut()
            .find(|offer| offer.connector.id == connector)
    }
}

fn advertise(device: LeaseDeviceId, offer: &mut Offer, resource: &WpDrmLeaseDeviceV1) -> bool {
    let Some(client) = Client::from_resource(resource) else {
        return false;
    };
    let instance = client.create::<WpDrmLeaseConnectorV1>(resource.version());
    ConnectorRef {
        device,
        connector: offer.connector.id,
    }
    .attach(&instance);
    resource.connector(&instance);
    instance.name(&offer.connector.name);
    instance.description(&offer.connector.description);
    instance.connector_id(offer.connector.id);
    instance.done();
    offer.instances.push(instance);
    true
}
