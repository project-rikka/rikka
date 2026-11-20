use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::WebPkiSupportedAlgorithms;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, Error, RootCertStore, SignatureScheme};
use webpki::{EndEntityCert, KeyUsage, VerifiedPath};

use crate::spki_pin::SpkiPin;
use crate::tls::pinning::{anchor_public_key_info, matches_pin};

#[derive(Debug)]
pub(crate) struct PinnedChain {
    validator: Arc<dyn ServerCertVerifier>,
    roots: Arc<RootCertStore>,
    algorithms: WebPkiSupportedAlgorithms,
    pins: Vec<SpkiPin>,
}

impl PinnedChain {
    pub(crate) fn new(
        validator: Arc<dyn ServerCertVerifier>,
        roots: Arc<RootCertStore>,
        algorithms: WebPkiSupportedAlgorithms,
        pins: Vec<SpkiPin>,
    ) -> PinnedChain {
        PinnedChain {
            validator,
            roots,
            algorithms,
            pins,
        }
    }

    fn pinned_path_exists(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> bool {
        let Ok(certificate) = EndEntityCert::try_from(end_entity) else {
            return false;
        };
        let pinned = |path: &VerifiedPath<'_>| match self.path_is_pinned(path) {
            true => Ok(()),
            false => Err(webpki::Error::UnknownIssuer),
        };

        certificate
            .verify_for_usage(
                self.algorithms.all,
                &self.roots.roots,
                intermediates,
                now,
                KeyUsage::server_auth(),
                None,
                Some(&pinned),
            )
            .is_ok()
    }

    fn path_is_pinned(&self, path: &VerifiedPath<'_>) -> bool {
        matches_pin(
            &self.pins,
            path.end_entity().subject_public_key_info().as_ref(),
        ) || path.intermediate_certificates().any(|certificate| {
            matches_pin(&self.pins, certificate.subject_public_key_info().as_ref())
        }) || matches_pin(&self.pins, &anchor_public_key_info(path.anchor()))
    }
}

impl ServerCertVerifier for PinnedChain {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, Error> {
        let verified = self.validator.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;

        match self.pinned_path_exists(end_entity, intermediates, now) {
            true => Ok(verified),
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
        self.validator.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, Error> {
        self.validator.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.validator.supported_verify_schemes()
    }
}
