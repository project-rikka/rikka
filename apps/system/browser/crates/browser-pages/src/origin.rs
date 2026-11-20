use std::fmt;

use url::Url;

use crate::hosts::host_key;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Origin {
    pub scheme: String,
    pub host: String,
    pub port: Option<u16>,
}

impl Origin {
    #[must_use]
    pub fn parse(uri: &str) -> Option<Origin> {
        let parsed = Url::parse(uri).ok()?;
        let host = match parsed.host()? {
            url::Host::Domain(domain) => host_key(domain),
            url::Host::Ipv4(address) => address.to_string(),
            url::Host::Ipv6(address) => address.to_string(),
        };
        if host.is_empty() {
            return None;
        }
        Some(Origin {
            scheme: parsed.scheme().to_owned(),
            host,
            port: parsed.port(),
        })
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}://", self.scheme)?;
        match self.host.contains(':') {
            true => write!(formatter, "[{}]", self.host)?,
            false => formatter.write_str(&self.host)?,
        }
        match self.port {
            Some(port) => write!(formatter, ":{port}"),
            None => Ok(()),
        }
    }
}
