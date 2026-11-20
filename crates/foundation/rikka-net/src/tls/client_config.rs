use std::sync::Arc;

use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::ServerCertVerifier;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::CertificateDer;
use rustls::{ClientConfig, RootCertStore};

use crate::error::{Error, Result};
use crate::tls::TlsConfig;
use crate::tls::pinned_chain::PinnedChain;
use crate::tls::pinned_leaf::PinnedLeaf;
use crate::tls::pinning::Pinning;

pub(super) fn client_config(config: &TlsConfig) -> Result<ClientConfig> {
    let provider = CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()));

    let builder = ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
        .map_err(|error| Error::Tls(Box::new(error)))?;

    let verifier = verifier(config, provider)?;
    Ok(builder
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth())
}

fn verifier(
    config: &TlsConfig,
    provider: Arc<CryptoProvider>,
) -> Result<Arc<dyn ServerCertVerifier>> {
    if let Some(Pinning::Leaf(pins)) = &config.pinning {
        return Ok(Arc::new(PinnedLeaf::new(provider, pins.clone())));
    }

    let roots = Arc::new(roots(config)?);
    let algorithms = provider.signature_verification_algorithms;
    let webpki = WebPkiServerVerifier::builder_with_provider(Arc::clone(&roots), provider)
        .build()
        .map_err(|error| Error::Tls(Box::new(error)))?;
    let validator: Arc<dyn ServerCertVerifier> = webpki;

    Ok(match &config.pinning {
        Some(Pinning::Chain(pins)) => {
            Arc::new(PinnedChain::new(validator, roots, algorithms, pins.clone()))
        }
        _ => validator,
    })
}

fn roots(config: &TlsConfig) -> Result<RootCertStore> {
    let mut store = RootCertStore::empty();

    if config.system_roots {
        let loaded = rustls_native_certs::load_native_certs();
        for error in &loaded.errors {
            tracing::debug!(%error, "failed to load system certificate");
        }
        let (added, ignored) = store.add_parsable_certificates(loaded.certs);
        tracing::debug!(added, ignored, "loaded system trust store");
    }

    for certificate in &config.roots {
        store
            .add(CertificateDer::from(certificate.as_der().to_vec()))
            .map_err(|error| Error::Tls(Box::new(error)))?;
    }

    if store.is_empty() {
        return Err(Error::Tls(
            "there are no certificate authorities to trust".into(),
        ));
    }
    Ok(store)
}
