mod client_config;
mod pinned_chain;
mod pinned_leaf;
mod pinning;

use std::sync::{Arc, OnceLock};

use crate::certificate::Certificate;
use crate::error::Result;

pub(crate) use pinning::Pinning;

const PROTOCOLS: [&[u8]; 2] = [b"h2", b"http/1.1"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TlsConfig {
    pub(crate) roots: Vec<Certificate>,
    pub(crate) system_roots: bool,
    pub(crate) pinning: Option<Pinning>,
}

impl TlsConfig {
    pub(crate) fn apply_to_http(
        &self,
        builder: reqwest::ClientBuilder,
    ) -> Result<reqwest::ClientBuilder> {
        if self.pinning.is_some() {
            let mut config = client_config::client_config(self)?;
            config.alpn_protocols = PROTOCOLS.iter().map(|protocol| protocol.to_vec()).collect();
            return Ok(builder.tls_backend_preconfigured(config));
        }

        let roots = self
            .roots
            .iter()
            .map(Certificate::to_reqwest)
            .collect::<Result<Vec<_>>>()?;

        Ok(match self.system_roots {
            true => builder.tls_certs_merge(roots),
            false => builder.tls_certs_only(roots),
        })
    }

    fn is_default(&self) -> bool {
        *self == TlsConfig::default()
    }
}

#[derive(Debug)]
pub(crate) struct SocketTls {
    config: TlsConfig,
    built: OnceLock<Arc<rustls::ClientConfig>>,
}

impl SocketTls {
    pub(crate) fn new(config: TlsConfig) -> SocketTls {
        SocketTls {
            config,
            built: OnceLock::new(),
        }
    }

    pub(crate) fn client_config(&self) -> Result<Arc<rustls::ClientConfig>> {
        static DEFAULT: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
        let cell = match self.config.is_default() {
            true => &DEFAULT,
            false => &self.built,
        };
        if let Some(built) = cell.get() {
            return Ok(Arc::clone(built));
        }
        let built = Arc::new(client_config::client_config(&self.config)?);
        Ok(Arc::clone(cell.get_or_init(|| built)))
    }
}

impl Default for TlsConfig {
    fn default() -> TlsConfig {
        TlsConfig {
            roots: Vec::new(),
            system_roots: true,
            pinning: None,
        }
    }
}
