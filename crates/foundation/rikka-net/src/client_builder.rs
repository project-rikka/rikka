use std::fmt;
use std::time::Duration;

use http::{HeaderMap, HeaderName, HeaderValue};

use crate::certificate::Certificate;
use crate::client::{Client, Shared};
use crate::error::{Error, Result};
use crate::headers;
use crate::proxy::Proxy;
use crate::redirect::Redirect;
use crate::retry::RetryPolicy;
use crate::spki_pin::SpkiPin;
use crate::tls::{Pinning, SocketTls, TlsConfig};

const DEFAULT_USER_AGENT: &str = concat!("rikka-net/", env!("CARGO_PKG_VERSION"));

pub struct ClientBuilder {
    inner: reqwest::ClientBuilder,
    error: Option<Error>,
    headers: HeaderMap,
    retry: RetryPolicy,
    tls: TlsConfig,
    connect_timeout: Option<Duration>,
}

impl ClientBuilder {
    #[must_use]
    pub fn new() -> ClientBuilder {
        let inner = reqwest::ClientBuilder::new()
            .user_agent(DEFAULT_USER_AGENT)
            .gzip(true)
            .brotli(true)
            .zstd(true)
            .deflate(true)
            .cookie_store(true);

        ClientBuilder {
            inner,
            error: None,
            headers: HeaderMap::new(),
            retry: RetryPolicy::default(),
            tls: TlsConfig::default(),
            connect_timeout: None,
        }
    }

    #[must_use]
    pub fn user_agent(mut self, user_agent: impl AsRef<str>) -> Self {
        self.inner = self.inner.user_agent(user_agent.as_ref());
        self
    }

    #[must_use]
    pub fn header<K, V>(mut self, name: K, value: V) -> Self
    where
        K: TryInto<HeaderName>,
        V: TryInto<HeaderValue>,
    {
        match headers::pair(name, value) {
            Ok((name, value)) => {
                self.headers.insert(name, value);
                self
            }
            Err(error) => self.fail(error),
        }
    }

    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.timeout(timeout);
        self
    }

    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self.inner = self.inner.connect_timeout(timeout);
        self
    }

    #[must_use]
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.inner = self.inner.read_timeout(timeout);
        self
    }

    #[must_use]
    pub fn pool_max_idle_per_host(mut self, connections: usize) -> Self {
        self.inner = self.inner.pool_max_idle_per_host(connections);
        self
    }

    #[must_use]
    pub fn redirect(mut self, redirect: Redirect) -> Self {
        self.inner = self.inner.redirect(redirect.policy());
        self
    }

    #[must_use]
    pub fn referer(mut self, send: bool) -> Self {
        self.inner = self.inner.referer(send);
        self
    }

    #[must_use]
    pub fn cookies(mut self, store: bool) -> Self {
        self.inner = self.inner.cookie_store(store);
        self
    }

    #[must_use]
    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.inner = self.inner.proxy(proxy.into_inner());
        self
    }

    #[must_use]
    pub fn compression(mut self, decode: bool) -> Self {
        self.inner = self
            .inner
            .gzip(decode)
            .brotli(decode)
            .zstd(decode)
            .deflate(decode);
        self
    }

    #[must_use]
    pub fn https_only(mut self, only: bool) -> Self {
        self.inner = self.inner.https_only(only);
        self
    }

    #[must_use]
    pub fn root_certificate(mut self, certificate: Certificate) -> Self {
        self.tls.roots.push(certificate);
        self
    }

    #[must_use]
    pub fn only_root_certificates(
        mut self,
        certificates: impl IntoIterator<Item = Certificate>,
    ) -> Self {
        self.tls.roots = certificates.into_iter().collect();
        self.tls.system_roots = false;
        self
    }

    #[must_use]
    pub fn pin_public_keys(mut self, pins: impl IntoIterator<Item = SpkiPin>) -> Self {
        let pins: Vec<SpkiPin> = pins.into_iter().collect();
        self.tls.pinning = (!pins.is_empty()).then_some(Pinning::Chain(pins));
        self
    }

    #[must_use]
    pub fn trust_leaf_public_keys(mut self, pins: impl IntoIterator<Item = SpkiPin>) -> Self {
        let pins: Vec<SpkiPin> = pins.into_iter().collect();
        self.tls.pinning = (!pins.is_empty()).then_some(Pinning::Leaf(pins));
        self
    }

    #[must_use]
    pub fn resolve(mut self, host: &str, address: std::net::SocketAddr) -> Self {
        self.inner = self.inner.resolve(host, address);
        self
    }

    #[must_use]
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = policy;
        self
    }

    pub fn build(self) -> Result<Client> {
        if let Some(error) = self.error {
            return Err(error);
        }

        let inner = self
            .tls
            .apply_to_http(self.inner.default_headers(self.headers.clone()))?;

        let http = inner.build().map_err(|error| Error::Tls(Box::new(error)))?;
        let socket_tls = SocketTls::new(self.tls);

        Ok(Client::from_shared(Shared {
            http,
            socket_tls,
            retry: self.retry,
            headers: self.headers,
            connect_timeout: self.connect_timeout,
        }))
    }

    fn fail(mut self, error: Error) -> Self {
        if self.error.is_none() {
            self.error = Some(error);
        }
        self
    }
}

impl Default for ClientBuilder {
    fn default() -> ClientBuilder {
        ClientBuilder::new()
    }
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("headers", &self.headers)
            .field("retry", &self.retry)
            .field("tls", &self.tls)
            .finish_non_exhaustive()
    }
}
