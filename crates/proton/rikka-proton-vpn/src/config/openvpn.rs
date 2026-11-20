use rikka_proton_pgp::Secret;

use crate::certificate::Certificate;
use crate::config::protocol::Transmission;
use crate::error::Result;
use crate::key::KeyPair;

const CA: &str = include_str!("openvpn-ca.pem");
const TLS_CRYPT: &str = include_str!("openvpn-tls-crypt.key");
const PASSPHRASE_BYTES: usize = 16;

#[must_use]
#[derive(Debug, Clone)]
pub struct OpenVpnConfig {
    server_ip: String,
    ports: Vec<u16>,
    transmission: Transmission,
    domain: String,
    certificate: String,
    key: String,
    passphrase: Secret,
    ipv6: bool,
}

impl OpenVpnConfig {
    pub fn new(
        server_ip: impl Into<String>,
        domain: impl Into<String>,
        certificate: &Certificate,
        keys: &KeyPair,
        passphrase: &Secret,
    ) -> Result<OpenVpnConfig> {
        Ok(OpenVpnConfig {
            server_ip: server_ip.into(),
            ports: Vec::new(),
            transmission: Transmission::Udp,
            domain: domain.into(),
            certificate: certificate.certificate.clone(),
            key: keys.encrypted_private_key_pem(passphrase.expose())?,
            passphrase: passphrase.clone(),
            ipv6: false,
        })
    }

    #[must_use]
    pub fn generate_passphrase() -> Secret {
        Secret::random_hex(PASSPHRASE_BYTES)
    }

    pub fn ports(mut self, ports: impl IntoIterator<Item = u16>) -> OpenVpnConfig {
        self.ports = ports.into_iter().collect();
        self
    }

    pub fn transmission(mut self, transmission: Transmission) -> OpenVpnConfig {
        self.transmission = transmission;
        self
    }

    pub fn ipv6(mut self, enabled: bool) -> OpenVpnConfig {
        self.ipv6 = enabled;
        self
    }

    #[must_use]
    pub fn server_ip(&self) -> &str {
        &self.server_ip
    }

    #[must_use]
    pub fn remote_ports(&self) -> &[u16] {
        &self.ports
    }

    #[must_use]
    pub fn uses_tcp(&self) -> bool {
        self.transmission == Transmission::Tcp
    }

    #[must_use]
    pub fn domain(&self) -> &str {
        &self.domain
    }

    #[must_use]
    pub fn certificate(&self) -> &str {
        &self.certificate
    }

    #[must_use]
    pub fn encrypted_key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn key_passphrase(&self) -> &Secret {
        &self.passphrase
    }

    #[must_use]
    pub fn uses_ipv6(&self) -> bool {
        self.ipv6
    }

    #[must_use]
    pub fn ca(&self) -> &'static str {
        CA
    }

    #[must_use]
    pub fn tls_crypt(&self) -> &'static str {
        TLS_CRYPT
    }
}
