use std::io;
use std::num::NonZeroU32;
use std::os::fd::OwnedFd;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseConnector {
    pub id: u32,
    pub name: String,
    pub description: String,
}

#[derive(Debug)]
pub struct LeaseGrant {
    pub lessee: NonZeroU32,
    pub fd: OwnedFd,
}

#[derive(Debug, thiserror::Error)]
pub enum LeaseRejected {
    #[error("no free CRTC for the requested connectors")]
    NoPipeline,
    #[error("kernel rejected the lease")]
    Kernel(#[from] io::Error),
}
