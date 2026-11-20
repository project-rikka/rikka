use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub struct Proxy(reqwest::Proxy);

impl Proxy {
    pub fn all(url: &str) -> Result<Proxy> {
        reqwest::Proxy::all(url).map(Proxy).map_err(invalid)
    }

    pub fn http(url: &str) -> Result<Proxy> {
        reqwest::Proxy::http(url).map(Proxy).map_err(invalid)
    }

    pub fn https(url: &str) -> Result<Proxy> {
        reqwest::Proxy::https(url).map(Proxy).map_err(invalid)
    }

    #[must_use]
    pub fn auth(self, username: &str, password: &str) -> Proxy {
        Proxy(self.0.basic_auth(username, password))
    }

    #[must_use]
    pub fn no_proxy(self, hosts: &str) -> Proxy {
        Proxy(self.0.no_proxy(reqwest::NoProxy::from_string(hosts)))
    }

    pub(crate) fn into_inner(self) -> reqwest::Proxy {
        self.0
    }
}

fn invalid(error: reqwest::Error) -> Error {
    Error::InvalidUrl(error.to_string())
}
