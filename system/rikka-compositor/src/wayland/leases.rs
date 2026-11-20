use std::collections::BTreeMap;
use std::num::NonZeroU32;

use rikka_compositor_output::{
    DrmLeaseHost, DrmLeases, LeaseConnector, LeaseDeviceId, LeaseGrant, LeaseRejected,
};
use rikka_compositor_server::DisplayHandle;

use crate::backend::{Backend, LeaseCard};
use crate::state::Compositor;

#[derive(Debug, Default)]
pub(crate) struct Leasing {
    pub(crate) leases: DrmLeases,
    devices: BTreeMap<LeaseCard, Offered>,
}

#[derive(Debug)]
struct Offered {
    device: LeaseDeviceId,
    connectors: Vec<u32>,
}

impl Leasing {
    fn card_of(&self, device: LeaseDeviceId) -> Option<LeaseCard> {
        self.devices
            .iter()
            .find(|(_, offered)| offered.device == device)
            .map(|(card, _)| *card)
    }
}

impl DrmLeaseHost for Compositor {
    fn drm_leases(&mut self) -> &mut DrmLeases {
        &mut self.leasing.leases
    }

    fn grant_lease(
        &mut self,
        device: LeaseDeviceId,
        connectors: &[u32],
    ) -> Result<LeaseGrant, LeaseRejected> {
        let card = self
            .leasing
            .card_of(device)
            .ok_or(LeaseRejected::NoPipeline)?;
        let Some(leases) = self.backend.leases() else {
            return Err(LeaseRejected::NoPipeline);
        };
        leases.lease(card, connectors)
    }

    fn revoke_lease(&mut self, device: LeaseDeviceId, lessee: NonZeroU32) {
        let Some(card) = self.leasing.card_of(device) else {
            return;
        };
        if let Some(leases) = self.backend.leases() {
            leases.end_lease(card, lessee);
        }
    }
}

impl Leasing {
    pub(crate) fn offer(&mut self, backend: &mut dyn Backend, display: &DisplayHandle<Compositor>) {
        let Some(leases) = backend.leases() else {
            return;
        };
        let present = leases.lease_cards();
        let gone: Vec<LeaseCard> = self
            .devices
            .keys()
            .filter(|card| !present.contains(card))
            .copied()
            .collect();
        for card in gone {
            if let Some(offered) = self.devices.remove(&card) {
                tracing::info!("GPU with leased outputs removed");
                self.leases.remove_device(offered.device);
            }
        }
        for card in present {
            self.offer_card(backend, display, card);
        }
    }

    fn offer_card(
        &mut self,
        backend: &mut dyn Backend,
        display: &DisplayHandle<Compositor>,
        card: LeaseCard,
    ) {
        let Some(leases) = backend.leases() else {
            return;
        };
        let connectors = leases.leasable(card);
        if !self.devices.contains_key(&card) {
            if connectors.is_empty() {
                return;
            }
            let Some(fd) = leases.lease_device(card) else {
                return;
            };
            let device = self.leases.add_device(display, fd);
            self.devices.insert(
                card,
                Offered {
                    device,
                    connectors: Vec::new(),
                },
            );
        }
        let Some(offered) = self.devices.get_mut(&card) else {
            return;
        };
        let device = offered.device;
        let gone: Vec<u32> = offered
            .connectors
            .iter()
            .filter(|offered_connector| {
                !connectors
                    .iter()
                    .any(|connector| connector.id == **offered_connector)
            })
            .copied()
            .collect();
        offered
            .connectors
            .retain(|offered_connector| !gone.contains(offered_connector));
        for connector in gone {
            if let Some(lease) = self.leases.lease_of(device, connector)
                && let Some(lessee) = self.leases.end(lease)
                && let Some(leases) = backend.leases()
            {
                leases.end_lease(card, lessee);
            }
            self.leases.withdraw(device, connector);
        }
        for connector in connectors {
            let Some(offered) = self.devices.get_mut(&card) else {
                return;
            };
            if offered.connectors.contains(&connector.id) {
                continue;
            }
            tracing::info!("{} offered for lease", connector.name);
            offered.connectors.push(connector.id);
            self.leases.offer(device, connector);
        }
    }
}

pub(crate) fn connector(id: u32, name: &str, make: &str, model: &str) -> LeaseConnector {
    LeaseConnector {
        id,
        name: name.to_owned(),
        description: format!("{make} {model}"),
    }
}
