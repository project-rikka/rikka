use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::hosted_part::HostedPart;
use crate::messages;
use crate::vendor::Vendor;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Stored", into = "Stored")]
pub struct Hosted {
    vendor: Vendor,
    payload: Value,
    part: HostedPart,
    bytes: usize,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    vendor: Vendor,
    payload: Value,
}

impl Hosted {
    pub(crate) fn read(vendor: Vendor, payload: Value) -> Option<Hosted> {
        let part = match vendor {
            Vendor::Anthropic => messages::hosted_part(&payload)?,
            _ => return None,
        };
        let bytes = payload.to_string().len();
        Some(Hosted {
            vendor,
            payload,
            part,
            bytes,
        })
    }

    #[must_use]
    pub fn vendor(&self) -> Vendor {
        self.vendor
    }

    #[must_use]
    pub fn part(&self) -> &HostedPart {
        &self.part
    }

    #[must_use]
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub(crate) fn replay(&self, vendor: Vendor) -> Option<&Value> {
        (self.vendor == vendor).then_some(&self.payload)
    }
}

impl TryFrom<Stored> for Hosted {
    type Error = String;

    fn try_from(stored: Stored) -> Result<Hosted, String> {
        let vendor = stored.vendor;
        Hosted::read(vendor, stored.payload)
            .ok_or_else(|| format!("unreadable block run by {vendor}"))
    }
}

impl From<Hosted> for Stored {
    fn from(hosted: Hosted) -> Stored {
        Stored {
            vendor: hosted.vendor,
            payload: hosted.payload,
        }
    }
}
