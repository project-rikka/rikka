use std::io;
use std::sync::Arc;

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::{AlertDescription, ClientConfig, RootCertStore};

use crate::error::{Error, Result};
use crate::key::KeyPair;

pub(crate) const PROTON_ROOTS: &str = include_str!("roots.pem");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Failure {
    ServerCertificate,
    ClientCertificateExpired,
    ClientCertificateUnknownCa,
    Unreachable,
    Other,
}

pub(crate) fn client_config(
    certificate: &str,
    keys: &KeyPair,
    roots: &str,
) -> Result<Arc<ClientConfig>> {
    let mut store = RootCertStore::empty();
    for root in CertificateDer::pem_slice_iter(roots.as_bytes()) {
        let root = root.map_err(|_| Error::AgentRoots)?;
        store.add(root).map_err(|_| Error::AgentRoots)?;
    }
    if store.is_empty() {
        return Err(Error::AgentRoots);
    }

    let chain = CertificateDer::pem_slice_iter(certificate.as_bytes())
        .collect::<std::result::Result<Vec<CertificateDer<'static>>, _>>()
        .map_err(|_| Error::AgentCertificate {
            reason: "invalid PEM".to_owned(),
        })?;
    if chain.is_empty() {
        return Err(Error::AgentCertificate {
            reason: "no certificate in PEM".to_owned(),
        });
    }
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(keys.private_key_der().to_vec()));

    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(certificate_error)?
        .with_root_certificates(store)
        .with_client_auth_cert(chain, key)
        .map_err(certificate_error)?;

    Ok(Arc::new(config))
}

pub(crate) fn classify(error: &io::Error) -> Failure {
    let tls = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<rustls::Error>());
    if let Some(tls) = tls {
        return match tls {
            rustls::Error::InvalidCertificate(_) => Failure::ServerCertificate,
            rustls::Error::AlertReceived(AlertDescription::CertificateExpired) => {
                Failure::ClientCertificateExpired
            }
            rustls::Error::AlertReceived(AlertDescription::UnknownCA) => {
                Failure::ClientCertificateUnknownCa
            }
            _ => Failure::Other,
        };
    }
    match error.kind() {
        io::ErrorKind::ConnectionRefused
        | io::ErrorKind::TimedOut
        | io::ErrorKind::HostUnreachable
        | io::ErrorKind::NetworkUnreachable => Failure::Unreachable,
        _ => Failure::Other,
    }
}

fn certificate_error(error: rustls::Error) -> Error {
    Error::AgentCertificate {
        reason: error.to_string(),
    }
}
