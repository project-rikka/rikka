use serde::{Deserialize, Serialize};

use crate::config::NetShieldLevel;

const PARTNER: &str = "partner";

#[must_use]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct AgentFeatures {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    netshield_level: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    randomized_nat: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    split_tcp: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port_forwarding: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    safe_mode: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bouncing: Option<String>,
    #[serde(default, skip_serializing)]
    jail: Option<bool>,
    #[serde(default, skip_serializing)]
    forwarded_port: Option<u16>,
}

impl AgentFeatures {
    pub fn new() -> AgentFeatures {
        AgentFeatures::default()
    }

    pub fn netshield(mut self, level: NetShieldLevel) -> AgentFeatures {
        self.netshield_level = Some(level.level());
        self
    }

    pub fn vpn_accelerator(mut self, enabled: bool) -> AgentFeatures {
        self.split_tcp = Some(enabled);
        self
    }

    pub fn moderate_nat(mut self, enabled: bool) -> AgentFeatures {
        self.randomized_nat = Some(!enabled);
        self
    }

    pub fn port_forwarding(mut self, enabled: bool) -> AgentFeatures {
        self.port_forwarding = Some(enabled);
        self
    }

    pub fn safe_mode(mut self, enabled: bool) -> AgentFeatures {
        self.safe_mode = Some(enabled);
        self
    }

    pub fn bouncing(mut self, label: impl Into<String>) -> AgentFeatures {
        self.bouncing = Some(label.into());
        self
    }

    #[must_use]
    pub fn netshield_level(&self) -> Option<NetShieldLevel> {
        self.netshield_level.and_then(NetShieldLevel::from_level)
    }

    #[must_use]
    pub fn vpn_accelerator_enabled(&self) -> Option<bool> {
        self.split_tcp
    }

    #[must_use]
    pub fn moderate_nat_enabled(&self) -> Option<bool> {
        self.randomized_nat.map(|randomized| !randomized)
    }

    #[must_use]
    pub fn port_forwarding_enabled(&self) -> Option<bool> {
        self.port_forwarding
    }

    #[must_use]
    pub fn safe_mode_enabled(&self) -> Option<bool> {
        self.safe_mode
    }

    #[must_use]
    pub fn bouncing_label(&self) -> Option<&str> {
        self.bouncing.as_deref()
    }

    #[must_use]
    pub fn is_jailed(&self) -> Option<bool> {
        self.jail
    }

    #[must_use]
    pub fn forwarded_port(&self) -> Option<u16> {
        self.forwarded_port
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        let AgentFeatures {
            netshield_level,
            randomized_nat,
            split_tcp,
            port_forwarding,
            safe_mode,
            bouncing,
            ..
        } = self;
        netshield_level.is_none()
            && randomized_nat.is_none()
            && split_tcp.is_none()
            && port_forwarding.is_none()
            && safe_mode.is_none()
            && bouncing.is_none()
    }

    pub(crate) fn merge(&mut self, newer: &AgentFeatures) {
        merge(&mut self.netshield_level, newer.netshield_level);
        merge(&mut self.randomized_nat, newer.randomized_nat);
        merge(&mut self.split_tcp, newer.split_tcp);
        merge(&mut self.port_forwarding, newer.port_forwarding);
        merge(&mut self.safe_mode, newer.safe_mode);
        merge(&mut self.bouncing, newer.bouncing.clone());
    }

    pub(crate) fn differences_from(&self, current: &AgentFeatures) -> AgentFeatures {
        AgentFeatures {
            netshield_level: if_different(self.netshield_level, current.netshield_level),
            randomized_nat: if_different(self.randomized_nat, current.randomized_nat),
            split_tcp: if_different(self.split_tcp, current.split_tcp),
            port_forwarding: if_different(self.port_forwarding, current.port_forwarding),
            safe_mode: if_different(self.safe_mode, current.safe_mode),
            bouncing: if_different(self.bouncing.clone(), current.bouncing.clone()),
            jail: None,
            forwarded_port: None,
        }
    }

    pub(crate) fn clear_sent(&mut self, sent: &AgentFeatures) {
        clear_if(&mut self.netshield_level, sent.netshield_level.is_some());
        clear_if(&mut self.randomized_nat, sent.randomized_nat.is_some());
        clear_if(&mut self.split_tcp, sent.split_tcp.is_some());
        clear_if(&mut self.port_forwarding, sent.port_forwarding.is_some());
        clear_if(&mut self.safe_mode, sent.safe_mode.is_some());
        clear_if(&mut self.bouncing, sent.bouncing.is_some());
    }

    pub(crate) fn bounces_through_partner(&self) -> bool {
        self.bouncing.as_deref() == Some(PARTNER)
    }
}

fn merge<T>(slot: &mut Option<T>, newer: Option<T>) {
    if newer.is_some() {
        *slot = newer;
    }
}

fn if_different<T: PartialEq>(desired: Option<T>, current: Option<T>) -> Option<T> {
    desired.filter(|value| current.as_ref() != Some(value))
}

fn clear_if<T>(slot: &mut Option<T>, sent: bool) {
    if sent {
        *slot = None;
    }
}
