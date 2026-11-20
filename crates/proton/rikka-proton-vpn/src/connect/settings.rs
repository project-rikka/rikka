use crate::agent::AgentFeatures;
use crate::certificate::CertificateRequest;
use crate::config::NetShieldLevel;
use crate::key::KeyPair;

#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionSettings {
    netshield: NetShieldLevel,
    vpn_accelerator: bool,
    moderate_nat: bool,
    port_forwarding: bool,
    safe_mode: Option<bool>,
    ipv6: bool,
    device_name: Option<String>,
}

impl Default for ConnectionSettings {
    fn default() -> ConnectionSettings {
        ConnectionSettings {
            netshield: NetShieldLevel::Off,
            vpn_accelerator: true,
            moderate_nat: false,
            port_forwarding: false,
            safe_mode: None,
            ipv6: false,
            device_name: None,
        }
    }
}

impl ConnectionSettings {
    pub fn new() -> ConnectionSettings {
        ConnectionSettings::default()
    }

    pub fn netshield(mut self, level: NetShieldLevel) -> ConnectionSettings {
        self.netshield = level;
        self
    }

    pub fn vpn_accelerator(mut self, enabled: bool) -> ConnectionSettings {
        self.vpn_accelerator = enabled;
        self
    }

    pub fn moderate_nat(mut self, enabled: bool) -> ConnectionSettings {
        self.moderate_nat = enabled;
        self
    }

    pub fn port_forwarding(mut self, enabled: bool) -> ConnectionSettings {
        self.port_forwarding = enabled;
        self
    }

    pub fn safe_mode(mut self, enabled: bool) -> ConnectionSettings {
        self.safe_mode = Some(enabled);
        self
    }

    pub fn ipv6(mut self, enabled: bool) -> ConnectionSettings {
        self.ipv6 = enabled;
        self
    }

    pub fn device_name(mut self, name: impl Into<String>) -> ConnectionSettings {
        self.device_name = Some(name.into());
        self
    }

    #[must_use]
    pub fn uses_ipv6(&self) -> bool {
        self.ipv6
    }

    pub fn agent_features(&self, bouncing: Option<&str>) -> AgentFeatures {
        let mut features = AgentFeatures::new()
            .netshield(self.netshield)
            .vpn_accelerator(self.vpn_accelerator)
            .moderate_nat(self.moderate_nat)
            .port_forwarding(self.port_forwarding);
        if let Some(safe_mode) = self.safe_mode {
            features = features.safe_mode(safe_mode);
        }
        if let Some(label) = bouncing {
            features = features.bouncing(label);
        }
        features
    }

    pub(crate) fn certificate_request(&self, keys: &KeyPair) -> CertificateRequest {
        let request = CertificateRequest::new(keys);
        match &self.device_name {
            Some(name) => request.device_name(name.clone()),
            None => request,
        }
    }
}
