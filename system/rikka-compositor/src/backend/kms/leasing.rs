use std::num::NonZeroU32;
use std::os::fd::OwnedFd;

use rikka_compositor_kms::{Connector, Error as KmsError};
use rikka_compositor_output::{LeaseConnector, LeaseGrant, LeaseRejected, open_lease_device};

use super::Kms;
use super::connectors::UNKNOWN;
use crate::backend::{LeaseCard, Leases};
use crate::wayland::lease_connector;

impl Leases for Kms {
    fn lease_cards(&self) -> Vec<LeaseCard> {
        self.cards.keys().copied().collect()
    }

    fn lease_device(&self, card: LeaseCard) -> Option<OwnedFd> {
        let path = self.cards.get(&card)?.gpu().path();
        match open_lease_device(path) {
            Ok(fd) => Some(fd),
            Err(error) => {
                tracing::info!("cannot offer {} for leasing: {error}", path.display());
                None
            }
        }
    }

    fn leasable(&self, card: LeaseCard) -> Vec<LeaseConnector> {
        let Some(card) = self.cards.get(&card) else {
            return Vec::new();
        };
        card.connectors
            .leasable()
            .iter()
            .map(|connector| {
                let edid = connector.edid();
                lease_connector(
                    connector.id().raw(),
                    connector.name(),
                    edid.map_or(UNKNOWN, |edid| edid.make.as_str()),
                    edid.map_or(UNKNOWN, |edid| edid.model.as_str()),
                )
            })
            .collect()
    }

    fn lease(&mut self, card: LeaseCard, connectors: &[u32]) -> Result<LeaseGrant, LeaseRejected> {
        let Some(card) = self.cards.get_mut(&card) else {
            return Err(LeaseRejected::NoPipeline);
        };
        let matching: Vec<&Connector> = card
            .connectors
            .leasable()
            .iter()
            .filter(|connector| connectors.contains(&connector.id().raw()))
            .collect();
        if matching.len() != connectors.len() {
            return Err(LeaseRejected::NoPipeline);
        }
        match card.device.lease(&matching) {
            Ok(lease) => {
                tracing::info!(
                    "lessee {} granted {} output(s)",
                    lease.lessee,
                    matching.len()
                );
                Ok(LeaseGrant {
                    lessee: lease.lessee,
                    fd: lease.fd,
                })
            }
            Err(KmsError::Lease(error)) => Err(LeaseRejected::Kernel(error)),
            Err(error) => {
                tracing::info!("cannot grant lease: {error}");
                Err(LeaseRejected::NoPipeline)
            }
        }
    }

    fn end_lease(&mut self, card: LeaseCard, lessee: NonZeroU32) {
        let Some(card) = self.cards.get_mut(&card) else {
            return;
        };
        match card.device.end_lease(lessee) {
            Ok(()) => tracing::info!("lessee {lessee} released its lease"),
            Err(error) => tracing::warn!("cannot revoke lessee {lessee}: {error}"),
        }
    }
}
