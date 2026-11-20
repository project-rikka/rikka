mod roots;

use std::fmt;
use std::time::Duration;

use rikka_certificate_transparency::{LogList, Standing};
use rustls_pki_types::pem::PemObject as _;
use rustls_pki_types::{CertificateDer, SignatureVerificationAlgorithm, TrustAnchor, UnixTime};
use webpki::aws_lc_rs::{
    ECDSA_P256_SHA256, ECDSA_P256_SHA384, ECDSA_P384_SHA256, ECDSA_P384_SHA384, ED25519,
    RSA_PKCS1_2048_8192_SHA256, RSA_PKCS1_2048_8192_SHA384, RSA_PKCS1_2048_8192_SHA512,
};
use webpki::{EndEntityCert, KeyUsage};
use x509_cert::Certificate;
use x509_cert::der::Decode as _;
use x509_cert::der::asn1::ObjectIdentifier;
use x509_cert::ext::pkix::SubjectAltName;
use x509_cert::ext::pkix::name::GeneralName;

use crate::epoch::CertificateIssuer;
use crate::failure::Failure;

use roots::ROOTS;

const SUBJECT_ALTERNATIVE_NAME: ObjectIdentifier = ObjectIdentifier::new_unwrap("2.5.29.17");

const ALGORITHMS: &[&dyn SignatureVerificationAlgorithm] = &[
    ECDSA_P256_SHA256,
    ECDSA_P256_SHA384,
    ECDSA_P384_SHA256,
    ECDSA_P384_SHA384,
    ED25519,
    RSA_PKCS1_2048_8192_SHA256,
    RSA_PKCS1_2048_8192_SHA384,
    RSA_PKCS1_2048_8192_SHA512,
];

#[derive(Clone)]
pub struct EpochCertificate {
    chain: Vec<CertificateDer<'static>>,
    leaf: Certificate,
}

impl EpochCertificate {
    pub fn parse(pem: &str) -> Result<EpochCertificate, Failure> {
        let chain = CertificateDer::pem_slice_iter(pem.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| Failure::MalformedCertificate(error.to_string()))?;
        let first = chain
            .first()
            .ok_or_else(|| Failure::MalformedCertificate("empty certificate chain".to_owned()))?;
        let leaf = Certificate::from_der(first)
            .map_err(|error| Failure::MalformedCertificate(error.to_string()))?;
        Ok(EpochCertificate { chain, leaf })
    }

    #[must_use]
    pub fn not_before(&self) -> u64 {
        self.leaf
            .tbs_certificate
            .validity
            .not_before
            .to_unix_duration()
            .as_secs()
    }

    #[must_use]
    pub fn alternative_name(&self) -> Option<String> {
        let extension = self
            .leaf
            .tbs_certificate
            .extensions
            .iter()
            .flatten()
            .find(|extension| extension.extn_id == SUBJECT_ALTERNATIVE_NAME)?;
        let names = SubjectAltName::from_der(extension.extn_value.as_bytes()).ok()?;
        names
            .0
            .iter()
            .filter_map(|name| match name {
                GeneralName::DnsName(name) => Some(name.to_string()),
                _ => None,
            })
            .reduce(|longest, name| {
                if name.len() > longest.len() {
                    name
                } else {
                    longest
                }
            })
    }

    pub fn verify(
        &self,
        issuer: CertificateIssuer,
        log_list: &LogList,
        time: u64,
    ) -> Result<(), Failure> {
        let roots = ROOTS
            .of(issuer)
            .filter(|roots| !roots.is_empty())
            .ok_or(Failure::UnknownIssuer(issuer.into()))?;
        let anchors: Vec<TrustAnchor<'static>> =
            roots.iter().map(|root| root.anchor.clone()).collect();

        let (leaf, intermediates) = self
            .chain
            .split_first()
            .ok_or_else(|| Failure::MalformedCertificate("empty certificate chain".to_owned()))?;
        let end_entity = EndEntityCert::try_from(leaf)
            .map_err(|error| Failure::CertificateChain(error.to_string()))?;
        let path = end_entity
            .verify_for_usage(
                ALGORITHMS,
                &anchors,
                intermediates,
                UnixTime::since_unix_epoch(Duration::from_secs(time)),
                KeyUsage::server_auth(),
                None,
                None,
            )
            .map_err(|error| Failure::CertificateChain(error.to_string()))?;

        let issuer_public_key_info = match path.intermediate_certificates().next() {
            Some(intermediate) => intermediate.subject_public_key_info().as_ref().to_vec(),
            None => roots
                .iter()
                .find(|root| {
                    root.anchor.subject_public_key_info == path.anchor().subject_public_key_info
                })
                .map(|root| root.public_key_info.clone())
                .ok_or_else(|| {
                    Failure::CertificateChain("trust anchor is not pinned".to_owned())
                })?,
        };

        let scts = log_list
            .embedded_scts(leaf, &issuer_public_key_info)
            .map_err(|error| {
                tracing::debug!(%error, "unreadable epoch certificate timestamps");
                Failure::InsufficientTimestamps
            })?;
        if scts.iter().any(|sct| sct.standing() == Standing::Current) {
            Ok(())
        } else {
            Err(Failure::InsufficientTimestamps)
        }
    }
}

impl fmt::Debug for EpochCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EpochCertificate")
            .field("chain", &self.chain.len())
            .field("alternative_name", &self.alternative_name())
            .finish()
    }
}
