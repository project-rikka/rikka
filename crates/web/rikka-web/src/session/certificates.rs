use base64::Engine;
use rikka_gecko_protocol::rikka::certificates::{AllowCertificate, CertificateError};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Trust {
    Temporary,
    Permanent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedCertificate {
    pub host: String,
    pub port: u16,
    pub der: Vec<u8>,
    pub overridable: bool,
    pub(crate) uri: String,
}

impl FailedCertificate {
    pub(crate) fn from_wire(error: CertificateError) -> Option<FailedCertificate> {
        let der = base64::engine::general_purpose::STANDARD
            .decode(&error.certificate)
            .inspect_err(|decode| tracing::warn!(error = %decode, "failed to decode certificate"))
            .ok()?;
        Some(FailedCertificate {
            host: error.host,
            port: error.port,
            der,
            overridable: error.overridable,
            uri: error.uri,
        })
    }

    pub(crate) fn exception(&self, trust: Trust) -> AllowCertificate {
        AllowCertificate {
            host: self.host.clone(),
            port: self.port,
            certificate: base64::engine::general_purpose::STANDARD.encode(&self.der),
            permanent: trust == Trust::Permanent,
        }
    }
}
