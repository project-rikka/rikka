use std::num::NonZeroU32;
use std::os::fd::OwnedFd;

use rikka_compositor_output::{LeaseConnector, LeaseGrant, LeaseRejected};

pub(crate) type LeaseCard = u64;

pub(crate) trait Leases {
    fn lease_cards(&self) -> Vec<LeaseCard>;

    fn lease_device(&self, card: LeaseCard) -> Option<OwnedFd>;

    fn leasable(&self, card: LeaseCard) -> Vec<LeaseConnector>;

    fn lease(&mut self, card: LeaseCard, connectors: &[u32]) -> Result<LeaseGrant, LeaseRejected>;

    fn end_lease(&mut self, card: LeaseCard, lessee: NonZeroU32);
}
