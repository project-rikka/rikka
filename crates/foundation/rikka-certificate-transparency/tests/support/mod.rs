use jiff::Timestamp;
use rustls_pki_types::CertificateDer;
use rustls_pki_types::pem::PemObject as _;
use x509_cert::Certificate;
use x509_cert::der::{Decode as _, Encode as _};

const EPOCH_CHAIN: &[u8] = include_bytes!("../data/epoch_chain.pem");

pub(crate) fn at(timestamp: &str) -> Timestamp {
    timestamp.parse().unwrap()
}

pub(crate) struct EpochCertificate {
    pub(crate) leaf: Vec<u8>,
    pub(crate) issuer_public_key_info: Vec<u8>,
}

pub(crate) fn epoch_certificate() -> EpochCertificate {
    let chain: Vec<CertificateDer<'_>> = CertificateDer::pem_slice_iter(EPOCH_CHAIN)
        .collect::<Result<_, _>>()
        .unwrap();
    let issuer = Certificate::from_der(&chain[1]).unwrap();
    EpochCertificate {
        leaf: chain[0].to_vec(),
        issuer_public_key_info: issuer
            .tbs_certificate
            .subject_public_key_info
            .to_der()
            .unwrap(),
    }
}
