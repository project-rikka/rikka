use std::collections::HashMap;
use std::fmt;

use zbus::zvariant::Value;

use crate::error::Result;
use crate::tunnel::addressing::{self, Addressing};
use crate::tunnel::connection::{self, Keyed};
use crate::values::{self, Draft};

const FALLBACK_INTERFACE: &str = "wg0";
const EVERY_DOMAIN: &str = "~.";
const WIREGUARD: &str = "wireguard";
const SUBJECT: &str = "a WireGuard tunnel";

#[must_use]
#[derive(Clone, PartialEq, Eq)]
pub struct WireGuardTunnel {
    name: String,
    interface: String,
    private_key: String,
    listen_port: Option<u32>,
    mtu: Option<u32>,
    addresses: Vec<String>,
    dns: Vec<String>,
    exclusive_dns: bool,
    peers: Vec<WireGuardPeer>,
}

#[must_use]
#[derive(Clone, PartialEq, Eq)]
pub struct WireGuardPeer {
    public_key: String,
    endpoint: Option<String>,
    allowed_ips: Vec<String>,
    keepalive: Option<u32>,
    preshared_key: Option<String>,
}

impl WireGuardTunnel {
    pub fn new(name: impl Into<String>, private_key: impl Into<String>) -> WireGuardTunnel {
        let name = name.into();
        WireGuardTunnel {
            interface: interface_for(&name),
            name,
            private_key: private_key.into(),
            listen_port: None,
            mtu: None,
            addresses: Vec::new(),
            dns: Vec::new(),
            exclusive_dns: false,
            peers: Vec::new(),
        }
    }

    pub fn interface(mut self, interface: impl Into<String>) -> WireGuardTunnel {
        self.interface = interface.into();
        self
    }

    pub fn listen_port(mut self, port: u32) -> WireGuardTunnel {
        self.listen_port = Some(port);
        self
    }

    pub fn mtu(mut self, mtu: u32) -> WireGuardTunnel {
        self.mtu = Some(mtu);
        self
    }

    pub fn address(mut self, address: impl Into<String>) -> WireGuardTunnel {
        self.addresses.push(address.into());
        self
    }

    pub fn addresses(
        mut self,
        addresses: impl IntoIterator<Item = impl Into<String>>,
    ) -> WireGuardTunnel {
        self.addresses.extend(addresses.into_iter().map(Into::into));
        self
    }

    pub fn dns(mut self, servers: impl IntoIterator<Item = impl Into<String>>) -> WireGuardTunnel {
        self.dns.extend(servers.into_iter().map(Into::into));
        self
    }

    pub fn exclusive_dns(mut self, exclusive: bool) -> WireGuardTunnel {
        self.exclusive_dns = exclusive;
        self
    }

    pub fn peer(mut self, peer: WireGuardPeer) -> WireGuardTunnel {
        self.peers.push(peer);
        self
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub fn interface_name(&self) -> &str {
        &self.interface
    }

    #[must_use]
    pub fn peers(&self) -> &[WireGuardPeer] {
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

        let mut draft = connection::draft(&self.name, WIREGUARD, Some(&self.interface), user);

        values::put(
            &mut draft,
            WIREGUARD,
            "private-key",
            self.private_key.clone(),
        );
        if let Some(port) = self.listen_port {
            values::put(&mut draft, WIREGUARD, "listen-port", port);
        }
        if let Some(mtu) = self.mtu {
            values::put(&mut draft, WIREGUARD, "mtu", mtu);
        }
        let peers: Vec<HashMap<String, Value<'static>>> =
            self.peers.iter().map(WireGuardPeer::settings).collect();
        values::put(&mut draft, WIREGUARD, "peers", peers);

        addressing::write(
            &mut draft,
            &Addressing {
                addresses: &addresses,
                dns: &self.dns,
                search: EVERY_DOMAIN,
                exclusive: self.exclusive_dns,
            },
        );
        Ok(draft)
    }
}

impl WireGuardPeer {
    pub fn new(public_key: impl Into<String>) -> WireGuardPeer {
        WireGuardPeer {
            public_key: public_key.into(),
            endpoint: None,
            allowed_ips: Vec::new(),
            keepalive: None,
            preshared_key: None,
        }
    }

    pub fn endpoint(mut self, endpoint: impl Into<String>) -> WireGuardPeer {
        self.endpoint = Some(endpoint.into());
        self
    }

    pub fn allowed_ips(
        mut self,
        ranges: impl IntoIterator<Item = impl Into<String>>,
    ) -> WireGuardPeer {
        self.allowed_ips.extend(ranges.into_iter().map(Into::into));
        self
    }

    pub fn keepalive(mut self, seconds: u32) -> WireGuardPeer {
        self.keepalive = Some(seconds);
        self
    }

    pub fn preshared_key(mut self, key: impl Into<String>) -> WireGuardPeer {
        self.preshared_key = Some(key.into());
        self
    }

    #[must_use]
    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    #[must_use]
    pub fn endpoint_address(&self) -> Option<&str> {
        self.endpoint.as_deref()
    }

    fn settings(&self) -> HashMap<String, Value<'static>> {
        let mut peer = HashMap::from([(
            "public-key".to_owned(),
            Value::from(self.public_key.clone()),
        )]);
        if let Some(endpoint) = &self.endpoint {
            peer.insert("endpoint".to_owned(), Value::from(endpoint.clone()));
        }
        if !self.allowed_ips.is_empty() {
            peer.insert(
                "allowed-ips".to_owned(),
                Value::from(self.allowed_ips.clone()),
            );
        }
        if let Some(keepalive) = self.keepalive {
            peer.insert("persistent-keepalive".to_owned(), Value::from(keepalive));
        }
        if let Some(preshared) = &self.preshared_key {
            peer.insert("preshared-key".to_owned(), Value::from(preshared.clone()));
            peer.insert("preshared-key-flags".to_owned(), Value::from(0_u32));
        }
        peer
    }
}

impl fmt::Debug for WireGuardTunnel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WireGuardTunnel")
            .field("name", &self.name)
            .field("interface", &self.interface)
            .field("addresses", &self.addresses)
            .field("dns", &self.dns)
            .field("exclusive_dns", &self.exclusive_dns)
            .field("peers", &self.peers)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for WireGuardPeer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WireGuardPeer")
            .field("public_key", &self.public_key)
            .field("endpoint", &self.endpoint)
            .field("allowed_ips", &self.allowed_ips)
            .field("keepalive", &self.keepalive)
            .finish_non_exhaustive()
    }
}

pub(crate) fn interface_for(name: &str) -> String {
    addressing::interface_for(name, FALLBACK_INTERFACE)
}
