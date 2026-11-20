use serde::Deserialize;
use serde_json::Value;

use crate::agent::code::AgentCode;
use crate::agent::features::AgentFeatures;

const AD_BYTES: u64 = 200 * 1024;
const TRACKER_BYTES: u64 = 50 * 1024;
const MALWARE_BYTES: u64 = 750 * 1024;
const ADULT_BYTES: u64 = 1454 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JailState {
    Connected,
    Jailed,
    HardJailed,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentReason {
    pub code: AgentCode,
    pub is_final: bool,
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct ConnectionDetails {
    pub device_ip: Option<String>,
    pub device_country: Option<String>,
    pub server_ipv4: Option<String>,
    pub server_ipv6: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct NetShieldStats {
    #[serde(rename = "DNSBL/1b")]
    pub malware: u64,
    #[serde(rename = "DNSBL/2a")]
    pub ads: u64,
    #[serde(rename = "DNSBL/2b")]
    pub trackers: u64,
    #[serde(rename = "DNSBL/3a")]
    pub adult_content: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Restriction {
    P2p,
    Streaming,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentStatus {
    pub state: JailState,
    pub features: AgentFeatures,
    pub reason: Option<AgentReason>,
    pub switch_to: Option<String>,
    pub connection: Option<ConnectionDetails>,
    pub netshield: Option<NetShieldStats>,
    pub restrictions: Vec<Restriction>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct StatusMessage {
    state: String,
    #[serde(default)]
    features: Option<AgentFeatures>,
    #[serde(default)]
    reason: Option<ReasonMessage>,
    #[serde(default, rename = "please-switch-to")]
    switch_to: Option<String>,
    #[serde(default)]
    connection_details: Option<ConnectionDetails>,
    #[serde(default)]
    features_statistics: Option<Value>,
    #[serde(default)]
    restrictions: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReasonMessage {
    code: i64,
    #[serde(rename = "final", default)]
    is_final: bool,
    #[serde(default)]
    description: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ErrorMessage {
    pub(crate) code: i64,
    #[serde(default)]
    pub(crate) description: String,
}

impl NetShieldStats {
    #[must_use]
    pub fn saved_bytes(&self) -> u64 {
        self.ads * AD_BYTES
            + self.trackers * TRACKER_BYTES
            + self.malware * MALWARE_BYTES
            + self.adult_content * ADULT_BYTES
    }
}

impl JailState {
    fn parse(state: &str) -> JailState {
        match state {
            "connected" => JailState::Connected,
            "jailed" => JailState::Jailed,
            "hard-jailed" => JailState::HardJailed,
            other => JailState::Other(other.to_owned()),
        }
    }
}

impl From<StatusMessage> for AgentStatus {
    fn from(message: StatusMessage) -> AgentStatus {
        let netshield = message
            .features_statistics
            .as_ref()
            .and_then(|statistics| statistics.get("netshield-level"))
            .and_then(|stats| NetShieldStats::deserialize(stats).ok());
        let restrictions = message
            .restrictions
            .unwrap_or_default()
            .iter()
            .filter_map(|name| match name.to_ascii_lowercase().as_str() {
                "p2p" => Some(Restriction::P2p),
                "streaming" => Some(Restriction::Streaming),
                _ => None,
            })
            .collect();

        AgentStatus {
            state: JailState::parse(&message.state),
            features: message.features.unwrap_or_default(),
            reason: message.reason.map(|reason| AgentReason {
                code: AgentCode::from_code(reason.code),
                is_final: reason.is_final,
                description: reason.description,
            }),
            switch_to: message.switch_to.filter(|server| !server.is_empty()),
            connection: message.connection_details,
            netshield,
            restrictions,
        }
    }
}
