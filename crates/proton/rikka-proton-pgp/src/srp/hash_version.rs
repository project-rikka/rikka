use proton_srp::SrpHashVersion;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HashVersion {
    V0,
    V1,
    V2,
    V3,
    #[default]
    V4,
}

impl TryFrom<u8> for HashVersion {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(HashVersion::V0),
            1 => Ok(HashVersion::V1),
            2 => Ok(HashVersion::V2),
            3 => Ok(HashVersion::V3),
            4 => Ok(HashVersion::V4),
            _ => Err(Error::Invalid("unsupported SRP hash version")),
        }
    }
}

impl HashVersion {
    #[must_use]
    pub fn as_u8(self) -> u8 {
        match self {
            HashVersion::V0 => 0,
            HashVersion::V1 => 1,
            HashVersion::V2 => 2,
            HashVersion::V3 => 3,
            HashVersion::V4 => 4,
        }
    }

    #[must_use]
    pub fn needs_username(self) -> bool {
        matches!(self, HashVersion::V0 | HashVersion::V1 | HashVersion::V2)
    }
}

impl From<HashVersion> for SrpHashVersion {
    fn from(version: HashVersion) -> Self {
        match version {
            HashVersion::V0 => SrpHashVersion::V0,
            HashVersion::V1 => SrpHashVersion::V1,
            HashVersion::V2 => SrpHashVersion::V2,
            HashVersion::V3 => SrpHashVersion::V3,
            HashVersion::V4 => SrpHashVersion::V4,
        }
    }
}

impl From<SrpHashVersion> for HashVersion {
    fn from(version: SrpHashVersion) -> Self {
        match version {
            SrpHashVersion::V0 => HashVersion::V0,
            SrpHashVersion::V1 => HashVersion::V1,
            SrpHashVersion::V2 => HashVersion::V2,
            SrpHashVersion::V3 => HashVersion::V3,
            SrpHashVersion::V4 => HashVersion::V4,
        }
    }
}
