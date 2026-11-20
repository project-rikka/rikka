use std::fmt;
use std::path::Path;

use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use sha2::{Digest, Sha256};

use crate::error::LinkError;

const SUBJECT: &str = "rikka-remote";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pin([u8; 32]);

impl Pin {
    #[must_use]
    pub fn of_public_key(spki_der: &[u8]) -> Pin {
        Pin(Sha256::digest(spki_der).into())
    }

    pub fn of_certificate(certificate: &CertificateDer<'_>) -> Result<Pin, LinkError> {
        let parsed = webpki::EndEntityCert::try_from(certificate)
            .map_err(|error| LinkError::Certificate(error.to_string()))?;
        Ok(Pin::of_public_key(
            parsed.subject_public_key_info().as_ref(),
        ))
    }

    #[must_use]
    pub fn to_hex(self) -> String {
        rikka_types::hex::encode(self.0)
    }

    #[must_use]
    pub fn from_hex(text: &str) -> Option<Pin> {
        let mut bytes = [0_u8; 32];
        rikka_types::hex::decode_to_slice(text, &mut bytes).ok()?;
        Some(Pin(bytes))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for Pin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Pin({})", self.to_hex())
    }
}

impl fmt::Display for Pin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex = self.to_hex();
        let groups: Vec<&str> = (0..hex.len())
            .step_by(4)
            .map(|start| &hex[start..start + 4])
            .collect();
        formatter.write_str(&groups.join(":"))
    }
}

pub struct ServerIdentity {
    certificate: CertificateDer<'static>,
    key: PrivateKeyDer<'static>,
    pin: Pin,
}

impl ServerIdentity {
    pub fn load_or_create(path: &Path, host: &str) -> Result<ServerIdentity, LinkError> {
        match std::fs::read(path) {
            Ok(pem) => ServerIdentity::parse(&pem).map_err(|error| stored(path, error)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ServerIdentity::create(path, host)
            }
            Err(error) => Err(LinkError::Identity {
                path: path.to_path_buf(),
                error,
            }),
        }
    }

    #[must_use]
    pub fn pin(&self) -> Pin {
        self.pin
    }

    pub(crate) fn certificate(&self) -> CertificateDer<'static> {
        self.certificate.clone()
    }

    pub(crate) fn key(&self) -> PrivateKeyDer<'static> {
        self.key.clone_key()
    }

    fn parse(pem: &[u8]) -> Result<ServerIdentity, LinkError> {
        let certificate = CertificateDer::from_pem_slice(pem)
            .map_err(|error| LinkError::Certificate(error.to_string()))?;
        let key = PrivateKeyDer::from_pem_slice(pem)
            .map_err(|error| LinkError::Certificate(error.to_string()))?;
        let pin = Pin::of_certificate(&certificate)?;
        Ok(ServerIdentity {
            certificate,
            key,
            pin,
        })
    }

    fn create(path: &Path, host: &str) -> Result<ServerIdentity, LinkError> {
        let names = vec![SUBJECT.to_owned(), host.to_owned()];
        let made = rcgen::generate_simple_self_signed(names)
            .map_err(|error| LinkError::Certificate(error.to_string()))?;
        let pem = format!("{}{}", made.signing_key.serialize_pem(), made.cert.pem());
        write_private(path, pem.as_bytes())?;
        tracing::info!(path = %path.display(), "made a new remote desktop identity");
        ServerIdentity::parse(pem.as_bytes())
    }
}

impl fmt::Debug for ServerIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServerIdentity")
            .field("pin", &self.pin)
            .finish_non_exhaustive()
    }
}

fn write_private(path: &Path, contents: &[u8]) -> Result<(), LinkError> {
    rikka_fs::Replace::new(path)
        .private()
        .write(contents)
        .map_err(|error| LinkError::Identity {
            path: path.to_path_buf(),
            error,
        })
}

fn stored(path: &Path, error: LinkError) -> LinkError {
    match error {
        LinkError::Certificate(reason) => {
            LinkError::Certificate(format!("{}: {reason}", path.display()))
        }
        other => other,
    }
}
