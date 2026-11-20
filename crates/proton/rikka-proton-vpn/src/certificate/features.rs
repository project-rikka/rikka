use serde::Serialize;

use crate::config::NetShieldLevel;

#[must_use]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CertificateFeatures {
    #[serde(rename = "NetShieldLevel", skip_serializing_if = "Option::is_none")]
    netshield: Option<u8>,
    #[serde(rename = "SplitTCP", skip_serializing_if = "Option::is_none")]
    split_tcp: Option<bool>,
    #[serde(rename = "RandomNAT", skip_serializing_if = "Option::is_none")]
    random_nat: Option<bool>,
    #[serde(rename = "PortForwarding", skip_serializing_if = "Option::is_none")]
    port_forwarding: Option<bool>,
    #[serde(rename = "SafeMode", skip_serializing_if = "Option::is_none")]
    safe_mode: Option<bool>,
}

impl CertificateFeatures {
    pub fn new() -> CertificateFeatures {
        CertificateFeatures::default()
    }

    pub fn netshield(mut self, level: NetShieldLevel) -> CertificateFeatures {
        self.netshield = Some(level.level());
        self
    }

    pub fn split_tcp(mut self, enabled: bool) -> CertificateFeatures {
        self.split_tcp = Some(enabled);
        self
    }

    pub fn random_nat(mut self, enabled: bool) -> CertificateFeatures {
        self.random_nat = Some(enabled);
        self
    }

    pub fn port_forwarding(mut self, enabled: bool) -> CertificateFeatures {
        self.port_forwarding = Some(enabled);
        self
    }

    pub fn safe_mode(mut self, enabled: bool) -> CertificateFeatures {
        self.safe_mode = Some(enabled);
        self
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == CertificateFeatures::default()
    }
}
