use rikka_omnibox::UrlDisplay;
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DisplayAddress {
    pub(crate) host: String,
    pub(crate) rest: String,
}

pub(crate) fn display_address(address: &str) -> DisplayAddress {
    let plain = || DisplayAddress {
        host: String::new(),
        rest: address.to_owned(),
    };
    let Ok(url) = Url::parse(address) else {
        return plain();
    };
    if !matches!(url.scheme(), "http" | "https") {
        return plain();
    }
    let display = UrlDisplay::steady(&url);
    let Some(host) = display.host() else {
        return plain();
    };
    let text = display.text();
    DisplayAddress {
        host: text[host.clone()].to_owned(),
        rest: text[host.end..].to_owned(),
    }
}
