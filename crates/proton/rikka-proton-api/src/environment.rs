use std::fmt;

use url::Url;

use crate::error::{Error, Result};

const PRODUCTION: &str = "https://api.protonmail.ch/";
const WEB_SUFFIX: &str = ".proton.me/api/";
const LEGACY_DOMAIN: &str = "protonmail.ch";
const PRODUCTION_DOMAIN: &str = "proton.me";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Environment {
    base: Url,
}

impl Environment {
    #[must_use]
    pub fn production() -> Environment {
        Environment {
            base: Url::parse(PRODUCTION).expect("production URL should parse"),
        }
    }

    pub fn web(product: impl AsRef<str>) -> Result<Environment> {
        Environment::custom(format!("https://{}{WEB_SUFFIX}", product.as_ref()))
    }

    pub fn black(host: impl AsRef<str>) -> Result<Environment> {
        Environment::custom(format!("https://api.{}", host.as_ref()))
    }

    pub fn custom(url: impl AsRef<str>) -> Result<Environment> {
        let url = url.as_ref();
        let mut base = Url::parse(url).map_err(|_| Error::InvalidUrl(url.to_owned()))?;

        if !matches!(base.scheme(), "http" | "https") || base.cannot_be_a_base() {
            return Err(Error::InvalidUrl(url.to_owned()));
        }

        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }

        base.set_query(None);
        base.set_fragment(None);

        Ok(Environment { base })
    }

    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.base
    }

    #[must_use]
    pub fn host(&self) -> &str {
        self.base.host_str().unwrap_or_default()
    }

    pub fn join(&self, path: impl AsRef<str>) -> Result<Url> {
        let path = path.as_ref().trim_start_matches('/');
        self.base
            .join(path)
            .map_err(|_| Error::InvalidUrl(path.to_owned()))
    }

    pub fn with_host(&self, host: impl AsRef<str>) -> Result<Environment> {
        let host = host.as_ref();
        let mut base = self.base.clone();
        base.set_host(Some(host))
            .map_err(|_| Error::InvalidUrl(host.to_owned()))?;
        Ok(Environment { base })
    }

    pub fn with_prefix(&self, prefix: impl AsRef<str>) -> Result<Environment> {
        let Some(domain) = self.host().strip_prefix("api.") else {
            return Ok(self.clone());
        };

        let domain = match domain {
            LEGACY_DOMAIN => PRODUCTION_DOMAIN,
            other => other,
        };

        self.with_host(format!("{}-api.{domain}", prefix.as_ref()))
    }
}

impl Default for Environment {
    fn default() -> Environment {
        Environment::production()
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.base.as_str())
    }
}
