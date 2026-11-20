use std::num::NonZeroU32;
use std::os::fd::OwnedFd;

use drm::control::Device as _;
use rustix::fs::OFlags;

use crate::claim::{claim_planes, pick_crtc};
use crate::connector::Connector;
use crate::device::Device;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Reserved {
    pub(crate) crtcs: Vec<usize>,
    pub(crate) planes: Vec<usize>,
}

#[derive(Debug)]
pub struct Lease {
    pub lessee: NonZeroU32,
    pub fd: OwnedFd,
}

impl Device {
    pub fn lease(&mut self, connectors: &[&Connector]) -> Result<Lease> {
        let mut taken = self.taken_crtcs();
        let mut claimed = self.claimed_planes();
        let facts = self.resources.plane_facts();
        let mut reserved = Reserved::default();
        let mut objects = Vec::with_capacity(connectors.len() * 3);
        for connector in connectors {
            let reachable = self.resources.crtc_mask(&connector.reachable_crtcs);
            let current = connector
                .current_crtc
                .and_then(|crtc| self.resources.crtc_index(crtc));
            let crtc = pick_crtc(current, reachable, taken)
                .ok_or_else(|| Error::NoCrtc(connector.name().to_owned()))?;
            let primary = claim_planes(&facts, crtc, &claimed)
                .ok_or_else(|| Error::NoPrimaryPlane(connector.name().to_owned()))?
                .primary;
            taken |= 1 << crtc;
            claimed[primary] = true;
            reserved.crtcs.push(crtc);
            reserved.planes.push(primary);
            objects.extend([
                connector.id().raw(),
                u32::from(self.resources.crtcs[crtc]),
                u32::from(self.resources.planes[primary].handle),
            ]);
        }
        let objects: Vec<NonZeroU32> = objects.into_iter().filter_map(NonZeroU32::new).collect();
        let (lessee, fd) = self
            .fd
            .create_lease(&objects, OFlags::CLOEXEC.bits())
            .map_err(Error::Lease)?;
        self.leased.insert(lessee, reserved);
        Ok(Lease { lessee, fd })
    }

    pub fn end_lease(&mut self, lessee: NonZeroU32) -> Result<()> {
        self.leased.remove(&lessee).ok_or(Error::UnknownLease)?;
        self.fd.revoke_lease(lessee).map_err(Error::Lease)
    }
}
