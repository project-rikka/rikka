use std::collections::HashMap;
use std::fmt;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::profile::dbus;
use crate::tunnel::addressing::{self, Addressing};
use crate::tunnel::connection::{self, Keyed};
use crate::values::{self, Draft};

pub(crate) const SERVICE: &str = "org.freedesktop.NetworkManager.protun";
const VPN: &str = "vpn";
const SUBJECT: &str = "a ProTun tunnel";
const FALLBACK_INTERFACE: &str = "proton0";
const SETTINGS_VERSION: u32 = 1;
const EVERY_DOMAIN: &str = "~";
const STORED_BY_NETWORK_MANAGER: &str = "0";
const NO_EXTERNAL_GATEWAY_ROUTE: i32 = 0;

#[must_use]
#[derive(Clone, PartialEq, Eq)]
pub struct ProtunTunnel {
    name: String,
    interface: String,
    private_key: String,
    addresses: Vec<String>,
    dns: Vec<String>,
    peers: Vec<ProtunPeer>,
}

#[must_use]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct ProtunPeer {
    id: String,
    endpoint: String,
    public_key: String,
    udp_ports: Vec<u16>,
    tcp_ports: Vec<u16>,
    tls_ports: Vec<u16>,
    priority: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
struct Settings<'a> {
    version: u32,
    peers: &'a [ProtunPeer],
    pcap_file: Option<String>,
}

impl ProtunTunnel {
    pub fn new(name: impl Into<String>, private_key: impl Into<String>) -> ProtunTunnel {
        ProtunTunnel {
            name: name.into(),
            interface: FALLBACK_INTERFACE.to_owned(),
            private_key: private_key.into(),
            addresses: Vec::new(),
            dns: Vec::new(),
            peers: Vec::new(),
        }
    }

    pub fn interface(mut self, interface: impl Into<String>) -> ProtunTunnel {
        self.interface = interface.into();
        self
    }

    pub fn addresses(
        mut self,
        addresses: impl IntoIterator<Item = impl Into<String>>,
    ) -> ProtunTunnel {
        self.addresses.extend(addresses.into_iter().map(Into::into));
        self
    }

    pub fn dns(mut self, servers: impl IntoIterator<Item = impl Into<String>>) -> ProtunTunnel {
        self.dns.extend(servers.into_iter().map(Into::into));
        self
    }

    pub fn peer(mut self, peer: ProtunPeer) -> ProtunTunnel {
        self.peers.push(peer);
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn peers(&self) -> &[ProtunPeer] {
        &self.peers
    }

    pub(crate) fn draft(&self, user: &str) -> Result<Draft> {
        Keyed {
            subject: SUBJECT,
            private_key: &self.private_key,
            interface: &self.interface,
            has_peers: !self.peers.is_empty(),
        }
        .check()?;
        let addresses = addressing::parse(SUBJECT, &self.addresses)?;
        if !addressing::has_family(&addresses, false) {
            return Err(Error::invalid(SUBJECT, "it has no IPv4 address"));
        }
        let settings = serde_json::to_string(&Settings {
            version: SETTINGS_VERSION,
            peers: &self.peers,
            pcap_file: None,
        })
        .map_err(|error| Error::invalid(SUBJECT, error.to_string()))?
        .replace(',', r"\,");
        let data = HashMap::from([
            ("settings".to_owned(), settings),
            (
                "private-key-flags".to_owned(),
                STORED_BY_NETWORK_MANAGER.to_owned(),
            ),
        ]);
        let secrets = HashMap::from([("private-key".to_owned(), self.private_key.clone())]);

        let mut draft = connection::draft(&self.name, VPN, Some(&self.interface), user);
        values::put(&mut draft, VPN, "service-type", SERVICE.to_owned());
        values::put(&mut draft, VPN, "data", data);
        values::put(&mut draft, VPN, "secrets", secrets);

        addressing::write(
            &mut draft,
            &Addressing {
                addresses: &addresses,
                dns: &self.dns,
                search: EVERY_DOMAIN,
                exclusive: true,
            },
        );
        for (setting, six) in [(dbus::IPV4, false), (dbus::IPV6, true)] {
            if addressing::has_family(&addresses, six) {
                values::put(
                    &mut draft,
                    setting,
                    "auto-route-ext-gw",
                    NO_EXTERNAL_GATEWAY_ROUTE,
                );
            }
        }
        Ok(draft)
    }
}

impl ProtunPeer {
    pub fn new(
        id: impl Into<String>,
        endpoint: impl Into<String>,
        public_key: impl Into<String>,
    ) -> ProtunPeer {
        ProtunPeer {
            id: id.into(),
            endpoint: endpoint.into(),
            public_key: public_key.into(),
            udp_ports: Vec::new(),
            tcp_ports: Vec::new(),
            tls_ports: Vec::new(),
            priority: 0,
        }
    }

    pub fn udp_ports(mut self, ports: impl IntoIterator<Item = u16>) -> ProtunPeer {
        self.udp_ports = ports.into_iter().collect();
        self
    }

    pub fn tcp_ports(mut self, ports: impl IntoIterator<Item = u16>) -> ProtunPeer {
        self.tcp_ports = ports.into_iter().collect();
        self
    }

    pub fn tls_ports(mut self, ports: impl IntoIterator<Item = u16>) -> ProtunPeer {
        self.tls_ports = ports.into_iter().collect();
        self
    }

    pub fn priority(mut self, priority: i32) -> ProtunPeer {
        self.priority = priority;
        self
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

impl fmt::Debug for ProtunTunnel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtunTunnel")
            .field("name", &self.name)
            .field("interface", &self.interface)
            .field("addresses", &self.addresses)
            .field("dns", &self.dns)
            .field("peers", &self.peers)
            .finish_non_exhaustive()
    }
}
