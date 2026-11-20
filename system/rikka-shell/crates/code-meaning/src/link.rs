use url::{Host, Url};

const FOREIGN: &str = "xn--";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Caution {
    Unencrypted,
    ForeignLetters,
    Credentials,
    NumericHost,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    address: String,
    host: String,
    cautions: Vec<Caution>,
}

impl Link {
    pub(super) fn read(text: &str) -> Option<Link> {
        let text = text.trim();
        if text.contains(char::is_whitespace) {
            return None;
        }
        let url = Url::parse(text).ok()?;
        let encrypted = match url.scheme() {
            "https" => true,
            "http" => false,
            _ => return None,
        };
        let host = url.host()?;
        let named = match &host {
            Host::Domain(name) => Some(*name),
            Host::Ipv4(_) | Host::Ipv6(_) => None,
        };
        let cautions = [
            (!encrypted).then_some(Caution::Unencrypted),
            named
                .is_some_and(|name| name.split('.').any(|label| label.starts_with(FOREIGN)))
                .then_some(Caution::ForeignLetters),
            (!url.username().is_empty() || url.password().is_some())
                .then_some(Caution::Credentials),
            named.is_none().then_some(Caution::NumericHost),
        ]
        .into_iter()
        .flatten()
        .collect();
        Some(Link {
            host: host.to_string(),
            address: url.into(),
            cautions,
        })
    }

    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    #[must_use]
    pub fn cautions(&self) -> &[Caution] {
        &self.cautions
    }
}
