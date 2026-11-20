use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error, SignatureScheme};
use webpki::EndEntityCert;

use crate::spki_pin::SpkiPin;
use crate::tls::pinning::matches_pin;

#[derive(Debug)]
pub(crate) struct PinnedLeaf {
    provider: Arc<CryptoProvider>,
    pins: Vec<SpkiPin>,
}

impl PinnedLeaf {
    pub(crate) fn new(provider: Arc<CryptoProvider>, pins: Vec<SpkiPin>) -> PinnedLeaf {
        PinnedLeaf { provider, pins }
    }
}

impl ServerCertVerifier for PinnedLeaf {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let certificate = EndEntityCert::try_from(end_entity)
            .map_err(|_| Error::InvalidCertificate(CertificateError::BadEncoding))?;

        match matches_pin(&self.pins, certificate.subject_public_key_info().as_ref()) {
            true => Ok(ServerCertVerified::assertion()),
            false => Err(Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            )),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
