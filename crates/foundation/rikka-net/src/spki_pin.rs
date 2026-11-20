use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rustls::pki_types::CertificateDer;
use sha2::{Digest, Sha256};

use crate::certificate::Certificate;
use crate::error::{Error, Result};

const DIGEST_LENGTH: usize = 32;
const PREFIX: &str = "sha256/";

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpkiPin([u8; DIGEST_LENGTH]);

impl SpkiPin {
    pub fn from_base64(text: impl AsRef<str>) -> Result<SpkiPin> {
        let text = text.as_ref().trim();
        let encoded = text.strip_prefix(PREFIX).unwrap_or(text);
        let digest = STANDARD
            .decode(encoded)
            .ok()
            .and_then(|bytes| <[u8; DIGEST_LENGTH]>::try_from(bytes).ok())
            .ok_or_else(|| Error::Tls(format!("\"{text}\" is not a SHA-256 key pin").into()))?;
        Ok(SpkiPin(digest))
    }

    #[must_use]
    pub fn from_sha256(digest: [u8; DIGEST_LENGTH]) -> SpkiPin {
        SpkiPin(digest)
    }

    #[must_use]
    pub fn from_public_key_info(der: &[u8]) -> SpkiPin {
        SpkiPin(Sha256::digest(der).into())
    }

    pub fn from_certificate(certificate: &Certificate) -> Result<SpkiPin> {
        let der = CertificateDer::from(certificate.as_der());
        let parsed = webpki::EndEntityCert::try_from(&der)
            .map_err(|error| Error::Tls(format!("failed to parse certificate: {error}").into()))?;
        Ok(SpkiPin::from_public_key_info(
            parsed.subject_public_key_info().as_ref(),
        ))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; DIGEST_LENGTH] {
        &self.0
    }

    #[must_use]
    pub fn to_base64(&self) -> String {
        STANDARD.encode(self.0)
    }
}

impl fmt::Debug for SpkiPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SpkiPin({PREFIX}{})", self.to_base64())
    }
}

impl fmt::Display for SpkiPin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{PREFIX}{}", self.to_base64())
    }
}
