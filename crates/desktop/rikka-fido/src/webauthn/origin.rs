use url::{Host, Url};

use crate::error::{Error, Result};

pub(crate) struct Origin {
    serialized: String,
    host: Host<String>,
}

impl Origin {
    pub(crate) fn parse(origin: &str) -> Result<Origin> {
        let invalid = || Error::InvalidOrigin(origin.to_owned());
        let url = Url::parse(origin).map_err(|_| invalid())?;
        let host = url.host().ok_or_else(invalid)?.to_owned();

        let local = matches!(&host, Host::Domain(domain) if is_localhost(domain));
        let secure = url.scheme() == "https" || (url.scheme() == "http" && local);
        if !secure || !matches!(url.path(), "" | "/") || url.query().is_some() {
            return Err(invalid());
        }

        Ok(Origin {
            serialized: url.origin().ascii_serialization(),
            host,
        })
    }

    pub(crate) fn serialized(&self) -> &str {
        &self.serialized
    }

    pub(crate) fn validate_rp_id(&self, rp_id: &str) -> Result<String> {
        let mismatch = || Error::RelyingPartyMismatch {
            rp_id: rp_id.to_owned(),
            origin: self.serialized.clone(),
        };
        if rp_id.is_empty() {
            return Err(mismatch());
        }
        let suffix = Host::parse(rp_id).map_err(|_| mismatch())?;

        if suffix == self.host {
            return Ok(suffix.to_string());
        }

        let (Host::Domain(host), Host::Domain(suffix)) = (&self.host, &suffix) else {
            return Err(mismatch());
        };
        if !is_registrable_suffix(host, suffix) {
            return Err(mismatch());
        }
        Ok(suffix.clone())
    }

    pub(crate) fn validate_appid(&self, appid: &str) -> Result<String> {
        let invalid = || Error::InvalidAppId(appid.to_owned());
        let url = Url::parse(appid).map_err(|_| invalid())?;
        let (Some(Host::Domain(appid_host)), Host::Domain(host)) = (url.host(), &self.host) else {
            return Err(invalid());
        };

        let same_site = rikka_public_suffix::registrable_domain(appid_host)
            .zip(rikka_public_suffix::registrable_domain(host))
            .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right));
        if url.scheme() != "https" || !same_site {
            return Err(invalid());
        }
        Ok(appid.to_owned())
    }
}

fn is_registrable_suffix(host: &str, suffix: &str) -> bool {
    if !host.ends_with(&format!(".{suffix}")) {
        return false;
    }
    let Some(own_suffix) = rikka_public_suffix::suffix(suffix) else {
        return false;
    };
    if own_suffix.as_str() == suffix {
        return false;
    }
    rikka_public_suffix::suffix(host)
        .is_none_or(|host_suffix| !host_suffix.as_str().ends_with(&format!(".{suffix}")))
}

fn is_localhost(domain: &str) -> bool {
    domain == "localhost" || domain.ends_with(".localhost")
}
