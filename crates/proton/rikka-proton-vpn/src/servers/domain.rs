use std::collections::BTreeMap;

use serde::Deserialize;

use rikka_proton_pgp::Secret;

use crate::certificate::Certificate;
use crate::config::{OpenVpnConfig, Protocol, ProtunConfig, WireGuardConfig};
use crate::error::{Error, Result};
use crate::key::KeyPair;
use crate::key::{X25519PublicKey, X25519SecretKey};

pub(crate) const ONLINE_STATUS: u8 = 1;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerEntryInfo {
    #[serde(rename = "IPv4")]
    pub ipv4: String,
    #[serde(rename = "Ports", default)]
    pub ports: Vec<u16>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectingDomain {
    #[serde(rename = "ID", default)]
    pub id: Option<String>,
    #[serde(rename = "Domain")]
    pub domain: String,
    #[serde(rename = "EntryIP", default)]
    pub entry_ip: Option<String>,
    #[serde(rename = "ExitIP", default)]
    pub exit_ip: Option<String>,
    #[serde(rename = "EntryPerProtocol", default)]
    pub entry_per_protocol: BTreeMap<String, ServerEntryInfo>,
    #[serde(rename = "Label", default)]
    pub label: Option<String>,
    #[serde(rename = "Status", default = "default_status")]
    pub status: u8,
    #[serde(rename = "Generation", default)]
    pub generation: Option<u32>,
    #[serde(rename = "ServicesDownReason", default)]
    pub services_down_reason: Option<String>,
    #[serde(rename = "X25519PublicKey", default)]
    pub public_key: Option<X25519PublicKey>,
}

#[derive(Debug, Clone)]
pub struct ServerEntry {
    protocol: Protocol,
    domain: String,
    label: Option<String>,
    ip: String,
    ports: Vec<u16>,
    public_key: Option<X25519PublicKey>,
}

fn default_status() -> u8 {
    ONLINE_STATUS
}

impl ConnectingDomain {
    #[must_use]
    pub fn is_online(&self) -> bool {
        self.status == ONLINE_STATUS
    }

    #[must_use]
    pub fn entry_ip(&self, protocol: Protocol) -> Option<&str> {
        self.entry_per_protocol
            .get(protocol.api_name())
            .map(|info| info.ipv4.as_str())
            .or(self.entry_ip.as_deref())
    }

    #[must_use]
    pub fn entry_ports(&self, protocol: Protocol) -> &[u16] {
        self.entry_per_protocol
            .get(protocol.api_name())
            .map_or(&[], |info| info.ports.as_slice())
    }

    #[must_use]
    pub fn entry(&self, protocol: Protocol) -> Option<ServerEntry> {
        Some(ServerEntry {
            protocol,
            domain: self.domain.clone(),
            label: self.label.clone().filter(|label| !label.is_empty()),
            ip: self.entry_ip(protocol)?.to_owned(),
            ports: self.entry_ports(protocol).to_vec(),
            public_key: self.public_key.clone(),
        })
    }
}

impl ServerEntry {
    #[must_use]
    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    #[must_use]
    pub fn ip(&self) -> &str {
        &self.ip
    }

    #[must_use]
    pub fn ports(&self) -> &[u16] {
        &self.ports
    }

    #[must_use]
    pub fn public_key(&self) -> Option<&X25519PublicKey> {
        self.public_key.as_ref()
    }

    #[must_use]
    pub fn endpoint(&self, port: u16) -> String {
        format!("{}:{port}", self.ip)
    }

    pub fn wireguard(&self, private_key: &X25519SecretKey, port: u16) -> Result<WireGuardConfig> {
        let public_key = self
            .public_key
            .as_ref()
            .ok_or_else(|| Error::MissingServerKey {
                server: self.domain.clone(),
            })?;

        Ok(WireGuardConfig::new(
            private_key,
            public_key,
            self.endpoint(port),
        ))
    }

    pub fn openvpn(
        &self,
        certificate: &Certificate,
        keys: &KeyPair,
        passphrase: &Secret,
        ports: impl IntoIterator<Item = u16>,
    ) -> Result<OpenVpnConfig> {
        Ok(OpenVpnConfig::new(
            self.ip.clone(),
            self.domain.clone(),
            certificate,
            keys,
            passphrase,
        )?
        .transmission(self.protocol.transmission())
        .ports(ports))
    }

    pub fn protun(
        &self,
        peer: impl Into<String>,
        private_key: &X25519SecretKey,
        ports: impl IntoIterator<Item = u16>,
    ) -> Result<ProtunConfig> {
        let ports: Vec<u16> = ports.into_iter().collect();
        let first = *ports.first().ok_or_else(|| Error::NoPort {
            server: self.domain.clone(),
            protocol: self.protocol,
        })?;
        let wireguard = self.wireguard(private_key, first)?;

        Ok(ProtunConfig::new(
            peer,
            self.ip.clone(),
            self.protocol.transmission(),
            wireguard,
        )
        .ports(ports))
    }
}
