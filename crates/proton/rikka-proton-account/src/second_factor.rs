use serde::{Deserialize, Serialize};

use crate::fido2::Fido2Info;

const ENABLED_TOTP: i32 = 0b01;
const ENABLED_FIDO2: i32 = 0b10;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecondFactorInfo {
    #[serde(rename = "Enabled", default)]
    enabled: i32,
    #[serde(rename = "FIDO2", default)]
    fido2: Fido2Info,
}

impl SecondFactorInfo {
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled != 0
    }

    #[must_use]
    pub fn has_totp(&self) -> bool {
        self.enabled & ENABLED_TOTP != 0
    }

    #[must_use]
    pub fn has_fido2(&self) -> bool {
        self.enabled & ENABLED_FIDO2 != 0
    }

    #[must_use]
    pub fn fido2(&self) -> Option<&Fido2Info> {
        self.has_fido2().then_some(&self.fido2)
    }
}
