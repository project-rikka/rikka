use std::num::NonZeroU32;

use super::store::DrmLeases;
use super::types::{LeaseGrant, LeaseRejected};
use crate::ids::LeaseDeviceId;

pub trait DrmLeaseHost: Sized + 'static {
    fn drm_leases(&mut self) -> &mut DrmLeases;

    fn grant_lease(
        &mut self,
        device: LeaseDeviceId,
        connectors: &[u32],
    ) -> Result<LeaseGrant, LeaseRejected>;

    fn revoke_lease(&mut self, device: LeaseDeviceId, lessee: NonZeroU32);
}
