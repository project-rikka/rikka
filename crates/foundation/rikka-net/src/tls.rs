use std::sync::{Arc, OnceLock};

use rustls::client::WebPkiServerVerifier;
use rustls::crypto::CryptoProvider;
use rustls::{ClientConfig, RootCertStore};

use crate::error::{Error, Result};

pub(crate) fn client_config() -> Result<Arc<ClientConfig>> {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    if let Some(built) = CONFIG.get() {
        return Ok(Arc::clone(built));
    }
    let built = Arc::new(build()?);
    Ok(Arc::clone(CONFIG.get_or_init(|| built)))
}

fn build() -> Result<ClientConfig> {
    let provider = CryptoProvider::get_default()
        .cloned()
        .unwrap_or_else(|| Arc::new(rustls::crypto::aws_lc_rs::default_provider()));

    let builder = ClientConfig::builder_with_provider(Arc::clone(&provider))
        .with_protocol_versions(&[&rustls::version::TLS12, &rustls::version::TLS13])
        .map_err(|error| Error::Tls(Box::new(error)))?;

    let verifier = WebPkiServerVerifier::builder_with_provider(Arc::new(system_roots()?), provider)
        .build()
        .map_err(|error| Error::Tls(Box::new(error)))?;
    Ok(builder
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth())
}

fn system_roots() -> Result<RootCertStore> {
    let mut store = RootCertStore::empty();
    let loaded = rustls_native_certs::load_native_certs();
    for error in &loaded.errors {
        tracing::debug!(%error, "failed to load system certificate");
    }
    let (added, ignored) = store.add_parsable_certificates(loaded.certs);
    tracing::debug!(added, ignored, "loaded system trust store");

    if store.is_empty() {
        return Err(Error::Tls(
            "there are no certificate authorities to trust".into(),
        ));
    }
    Ok(store)
}
