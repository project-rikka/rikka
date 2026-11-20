use rikka_sync::Mutex;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::WebPkiSupportedAlgorithms;
use rustls::{CertificateError, DigitallySignedStruct, SignatureScheme};
use rustls_pki_types::{CertificateDer, ServerName, UnixTime};

use crate::quic::identity::Pin;

#[derive(Clone, Debug, Default)]
pub struct Trust {
    expected: Option<Pin>,
    seen: Arc<Mutex<Option<Pin>>>,
}

impl Trust {
    #[must_use]
    pub fn first_use() -> Trust {
        Trust::default()
    }

    #[must_use]
    pub fn pinned(pin: Pin) -> Trust {
        Trust {
            expected: Some(pin),
            seen: Arc::default(),
        }
    }

    #[must_use]
    pub fn expected(&self) -> Option<Pin> {
        self.expected
    }

    #[must_use]
    pub fn seen(&self) -> Option<Pin> {
        *self.seen.lock()
    }

    #[must_use]
    pub fn mismatched(&self) -> bool {
        matches!((self.expected, self.seen()), (Some(expected), Some(seen)) if expected != seen)
    }
}

#[derive(Debug)]
pub(crate) struct PinVerifier {
    trust: Trust,
    algorithms: WebPkiSupportedAlgorithms,
}

impl PinVerifier {
    pub(crate) fn new(trust: Trust, algorithms: WebPkiSupportedAlgorithms) -> PinVerifier {
        PinVerifier { trust, algorithms }
    }
}

impl ServerCertVerifier for PinVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let pin = Pin::of_certificate(end_entity)
            .map_err(|_| rustls::Error::InvalidCertificate(CertificateError::BadEncoding))?;
        *self.trust.seen.lock() = Some(pin);
        match self.trust.expected {
            Some(expected) if expected != pin => Err(rustls::Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            )),
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, certificate, signature, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, certificate, signature, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}
