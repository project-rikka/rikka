use std::sync::LazyLock;

use rustls_pki_types::pem::PemObject as _;
use rustls_pki_types::{CertificateDer, TrustAnchor};
use x509_cert::Certificate;
use x509_cert::der::{Decode as _, Encode as _};

use crate::epoch::CertificateIssuer;

const LETS_ENCRYPT_ROOTS: &str = include_str!("../../data/lets_encrypt.pem");
const ZERO_SSL_ROOTS: &str = include_str!("../../data/zero_ssl.pem");

pub(crate) struct Root {
    pub(crate) anchor: TrustAnchor<'static>,
    pub(crate) public_key_info: Vec<u8>,
}

pub(crate) struct Roots {
    lets_encrypt: Vec<Root>,
    zero_ssl: Vec<Root>,
}

pub(crate) static ROOTS: LazyLock<Roots> = LazyLock::new(|| Roots {
    lets_encrypt: parse_roots(LETS_ENCRYPT_ROOTS),
    zero_ssl: parse_roots(ZERO_SSL_ROOTS),
});

impl Roots {
    pub(crate) fn of(&self, issuer: CertificateIssuer) -> Option<&[Root]> {
        match issuer {
            CertificateIssuer::LetsEncrypt => Some(&self.lets_encrypt),
            CertificateIssuer::ZeroSsl => Some(&self.zero_ssl),
            CertificateIssuer::Other(_) => None,
        }
    }
}

fn parse_roots(pem: &str) -> Vec<Root> {
    CertificateDer::pem_slice_iter(pem.as_bytes())
        .filter_map(|certificate| {
            let root = certificate
                .ok()
                .and_then(|certificate| parse_root(&certificate));
            if root.is_none() {
                tracing::error!("failed to parse pinned epoch root certificate");
            }
            root
        })
        .collect()
}

fn parse_root(certificate: &CertificateDer<'_>) -> Option<Root> {
    let anchor = webpki::anchor_from_trusted_cert(certificate)
        .ok()?
        .to_owned();
    let public_key_info = Certificate::from_der(certificate)
        .ok()?
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .ok()?;
    Some(Root {
        anchor,
        public_key_info,
    })
}
