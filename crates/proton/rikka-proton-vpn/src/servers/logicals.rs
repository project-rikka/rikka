use std::collections::BTreeMap;

use serde::Deserialize;

use crate::config::{Protocol, VpnProtocol};
use crate::error::{Error, Result};
use crate::servers::domain::{ConnectingDomain, ONLINE_STATUS, ServerEntry};
use crate::servers::features::{ServerFeatures, Tier};
use crate::servers::scoring::country_code;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct ServerLocation {
    #[serde(rename = "Latitude")]
    pub latitude: f32,
    #[serde(rename = "Longitude")]
    pub longitude: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct ServerStatusReference {
    #[serde(rename = "Index")]
    pub index: u32,
    #[serde(rename = "Penalty")]
    pub penalty: f64,
    #[serde(rename = "Cost")]
    pub cost: i32,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub struct LogicalsMetadata {
    #[serde(rename = "ListIsTruncated")]
    pub list_is_truncated: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogicalServer {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "EntryCountry")]
    pub entry_country: String,
    #[serde(rename = "ExitCountry")]
    pub exit_country: String,
    #[serde(rename = "HostCountry", default)]
    pub host_country: Option<String>,
    #[serde(rename = "Domain", default)]
    pub domain: Option<String>,
    #[serde(rename = "City", default)]
    pub city: Option<String>,
    #[serde(rename = "State", default)]
    pub state: Option<String>,
    #[serde(rename = "Tier", default)]
    pub tier: Tier,
    #[serde(rename = "Features", default)]
    pub features: ServerFeatures,
    #[serde(rename = "EntryLocation", default)]
    pub entry_location: Option<ServerLocation>,
    #[serde(rename = "ExitLocation", default)]
    pub exit_location: Option<ServerLocation>,
    #[serde(rename = "StatusReference", default)]
    pub status_reference: Option<ServerStatusReference>,
    #[serde(rename = "Load", default)]
    pub load: Option<u32>,
    #[serde(rename = "Score", default)]
    pub score: Option<f64>,
    #[serde(rename = "Status", default)]
    pub status: Option<u8>,
    #[serde(rename = "IsVisible", default = "default_visible")]
    pub visible: bool,
    #[serde(rename = "GatewayName", default)]
    pub gateway_name: Option<String>,
    #[serde(rename = "Translations", default)]
    pub translations: BTreeMap<String, Option<String>>,
    #[serde(rename = "Servers", default)]
    pub servers: Vec<ConnectingDomain>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Logicals {
    #[serde(rename = "StatusID", default)]
    pub status_id: Option<String>,
    #[serde(rename = "LogicalServers", default)]
    pub servers: Vec<LogicalServer>,
    #[serde(rename = "ResponseMetadata", default)]
    pub metadata: LogicalsMetadata,
}

impl LogicalServer {
    #[must_use]
    pub fn is_online(&self) -> bool {
        self.status.is_none_or(|status| status == ONLINE_STATUS)
            && self.servers.iter().any(ConnectingDomain::is_online)
    }

    #[must_use]
    pub fn exit_country_code(&self) -> String {
        country_code(&self.exit_country)
    }

    #[must_use]
    pub fn supports(&self, protocol: Protocol) -> bool {
        self.entries(protocol)
            .any(|entry| protocol.vpn() != VpnProtocol::WireGuard || entry.public_key().is_some())
    }

    pub fn entries(&self, protocol: Protocol) -> impl Iterator<Item = ServerEntry> + '_ {
        self.servers
            .iter()
            .filter(|domain| domain.is_online())
            .filter_map(move |domain| domain.entry(protocol))
    }

    pub fn entry(&self, protocol: Protocol) -> Result<ServerEntry> {
        self.entries(protocol)
            .find(|entry| protocol.vpn() != VpnProtocol::WireGuard || entry.public_key().is_some())
            .ok_or(Error::MissingEntry {
                server: self.name.clone(),
                protocol,
            })
    }
}

fn default_visible() -> bool {
    true
}
