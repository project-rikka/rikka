use std::fmt;
use std::num::NonZeroU32;
use std::os::fd::OwnedFd;

use rikka_compositor_protocols::wp_drm_lease_device_v1::WpDrmLeaseDeviceV1;
use rikka_compositor_protocols::wp_drm_lease_v1::WpDrmLeaseV1;
use rikka_compositor_server::DisplayHandle;
use slotmap::SlotMap;

use super::device::{Device, Offer};
use super::protocol::Pending;
use super::types::LeaseConnector;
use crate::ids::{LeaseDeviceId, LeaseId, LeaseRequestId};

pub struct DrmLeases {
    pub(super) devices: SlotMap<LeaseDeviceId, Device>,
    pub(super) leases: SlotMap<LeaseId, Lease>,
    pub(super) requests: SlotMap<LeaseRequestId, Pending>,
}

pub(super) struct Lease {
    pub(super) device: LeaseDeviceId,
    pub(super) resource: WpDrmLeaseV1,
    pub(super) lessee: NonZeroU32,
    pub(super) connectors: Vec<u32>,
}

impl DrmLeases {
    pub fn new() -> Self {
        Self {
            devices: SlotMap::with_key(),
            leases: SlotMap::with_key(),
            requests: SlotMap::with_key(),
        }
    }

    pub fn add_device<S: 'static>(
        &mut self,
        display: &DisplayHandle<S>,
        fd: OwnedFd,
    ) -> LeaseDeviceId {
        self.devices.insert_with_key(|id| {
            let global = display
                .global::<WpDrmLeaseDeviceV1>()
                .privileged()
                .key(id)
                .publish();
            Device {
                id,
                fd,
                offers: Vec::new(),
                bound: Vec::new(),
                global,
            }
        })
    }

    pub fn remove_device(&mut self, device: LeaseDeviceId) -> Vec<NonZeroU32> {
        let leases: Vec<LeaseId> = self
            .leases
            .iter()
            .filter(|(_, lease)| lease.device == device)
            .map(|(id, _)| id)
            .collect();
        let lessees = leases
            .into_iter()
            .filter_map(|lease| self.end(lease))
            .collect();
        if let Some(mut entry) = self.devices.remove(device) {
            let offered: Vec<u32> = entry
                .offers
                .iter()
                .map(|offer| offer.connector.id)
                .collect();
            entry.retract(&offered);
            entry.global.remove();
        }
        lessees
    }

    pub fn offer(&mut self, device: LeaseDeviceId, connector: LeaseConnector) {
        let Some(entry) = self.devices.get_mut(device) else {
            return;
        };
        if entry.offer_mut(connector.id).is_some() {
            return;
        }
        let id = connector.id;
        entry.offers.push(Offer {
            connector,
            leased: None,
            instances: Vec::new(),
        });
        entry.readvertise(&[id]);
    }

    pub fn withdraw(&mut self, device: LeaseDeviceId, connector: u32) {
        let Some(entry) = self.devices.get_mut(device) else {
            return;
        };
        entry.retract(&[connector]);
        entry.offers.retain(|offer| offer.connector.id != connector);
    }

    pub fn end(&mut self, lease: LeaseId) -> Option<NonZeroU32> {
        let lease = self.leases.remove(lease)?;
        if lease.resource.alive() {
            lease.resource.finished();
        }
        self.release(&lease);
        Some(lease.lessee)
    }

    pub fn lease_of(&self, device: LeaseDeviceId, connector: u32) -> Option<LeaseId> {
        self.leases
            .iter()
            .find(|(_, lease)| lease.device == device && lease.connectors.contains(&connector))
            .map(|(id, _)| id)
    }

    pub fn leases(&self) -> impl Iterator<Item = (LeaseId, LeaseDeviceId, &[u32])> {
        self.leases
            .iter()
            .map(|(id, lease)| (id, lease.device, lease.connectors.as_slice()))
    }

    pub(super) fn release(&mut self, lease: &Lease) {
        let Some(device) = self.devices.get_mut(lease.device) else {
            return;
        };
        for connector in &lease.connectors {
            if let Some(offer) = device.offer_mut(*connector) {
                offer.leased = None;
            }
        }
        device.readvertise(&lease.connectors);
    }
}

impl Default for DrmLeases {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for DrmLeases {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DrmLeases")
            .field("devices", &self.devices.len())
            .field("leases", &self.leases.len())
            .finish()
    }
}
