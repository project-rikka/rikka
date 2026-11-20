use std::fmt;

use crate::key::{X25519PublicKey, X25519SecretKey};

const CLIENT_IPV4: &str = "10.2.0.2/32";
const CLIENT_IPV6: &str = "2a07:b944::2:2/128";
const DNS_IPV4: &str = "10.2.0.1";
const DNS_IPV6: &str = "2a07:b944::2:1";
const ROUTE_IPV4: &str = "0.0.0.0/0";
const ROUTE_IPV6: &str = "::/0";
const DEFAULT_KEEPALIVE_SECONDS: u16 = 60;

#[must_use]
#[derive(Debug, Clone)]
pub struct WireGuardConfig {
    private_key: X25519SecretKey,
    public_key: X25519PublicKey,
    endpoint: String,
    ipv6: bool,
    dns: Vec<String>,
    allowed_ips: Vec<String>,
    keepalive: u16,
}

impl WireGuardConfig {
    pub fn new(
        private_key: &X25519SecretKey,
        public_key: &X25519PublicKey,
        endpoint: impl Into<String>,
    ) -> WireGuardConfig {
        WireGuardConfig {
            private_key: private_key.clone(),
            public_key: public_key.clone(),
            endpoint: endpoint.into(),
            ipv6: false,
            dns: Vec::new(),
            allowed_ips: Vec::new(),
            keepalive: DEFAULT_KEEPALIVE_SECONDS,
        }
    }

    pub fn ipv6(mut self, enabled: bool) -> WireGuardConfig {
        self.ipv6 = enabled;
        self
    }

    pub fn dns(mut self, servers: impl IntoIterator<Item = impl Into<String>>) -> WireGuardConfig {
        self.dns = servers.into_iter().map(Into::into).collect();
        self
    }

    pub fn allowed_ips(
        mut self,
        ranges: impl IntoIterator<Item = impl Into<String>>,
    ) -> WireGuardConfig {
        self.allowed_ips = ranges.into_iter().map(Into::into).collect();
        self
    }

    pub fn keepalive(mut self, seconds: u16) -> WireGuardConfig {
        self.keepalive = seconds;
        self
    }

    #[must_use]
    pub fn private_key(&self) -> &X25519SecretKey {
        &self.private_key
    }

    #[must_use]
    pub fn public_key(&self) -> &X25519PublicKey {
        &self.public_key
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    #[must_use]
    pub fn persistent_keepalive(&self) -> u16 {
        self.keepalive
    }

    #[must_use]
    pub fn addresses(&self) -> Vec<String> {
        self.per_ip_family(CLIENT_IPV4, CLIENT_IPV6)
    }

    #[must_use]
    pub fn dns_servers(&self) -> Vec<String> {
        match self.dns.is_empty() {
            true => self.per_ip_family(DNS_IPV4, DNS_IPV6),
            false => self.dns.clone(),
        }
    }

    #[must_use]
    pub fn routes(&self) -> Vec<String> {
        match self.allowed_ips.is_empty() {
            true => vec![ROUTE_IPV4.to_owned(), ROUTE_IPV6.to_owned()],
            false => self.allowed_ips.clone(),
        }
    }

    fn per_ip_family(&self, ipv4: &str, ipv6: &str) -> Vec<String> {
        match self.ipv6 {
            true => vec![ipv4.to_owned(), ipv6.to_owned()],
            false => vec![ipv4.to_owned()],
        }
    }
}

impl fmt::Display for WireGuardConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "[Interface]")?;
        writeln!(f, "PrivateKey = {}", self.private_key.to_base64())?;
        writeln!(f, "Address = {}", self.addresses().join(", "))?;
        writeln!(f, "DNS = {}", self.dns_servers().join(", "))?;
        writeln!(f)?;
        writeln!(f, "[Peer]")?;
        writeln!(f, "PublicKey = {}", self.public_key.to_base64())?;
        writeln!(f, "AllowedIPs = {}", self.routes().join(", "))?;
        writeln!(f, "Endpoint = {}", self.endpoint)?;
        writeln!(f, "PersistentKeepalive = {}", self.keepalive)
    }
}
