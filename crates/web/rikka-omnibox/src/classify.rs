use std::borrow::Cow;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;

use url::{Host, Url};

use crate::engines::SearchEngines;
use crate::fold::width_folded;
use crate::host::{self, HostClass};
use crate::search::Search;

const INTERNAL_PAGES: [&str; 4] = ["settings", "history", "downloads", "archive"];

const EXTERNAL_SCHEMES: [&str; 14] = [
    "mailto", "tel", "sms", "magnet", "webcal", "geo", "xmpp", "irc", "ircs", "news", "ssh",
    "sftp", "ftp", "matrix",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    text: String,
    intent: Intent,
    alternative: Option<Destination>,
    trailing_space: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    Empty,
    Navigate(Destination),
    Search(Search),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    url: Url,
    upgrade: Upgrade,
    bare: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Upgrade {
    Assumed,
    Available,
    Exempt,
    NotApplicable,
}

enum Resolved {
    Navigate(Destination),
    Search(Option<Destination>),
}

impl Classified {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    #[must_use]
    pub fn alternative(&self) -> Option<&Destination> {
        self.alternative.as_ref()
    }

    #[must_use]
    pub fn destination(&self) -> Option<&Destination> {
        match &self.intent {
            Intent::Navigate(destination) => Some(destination),
            _ => self.alternative.as_ref(),
        }
    }

    #[must_use]
    pub fn search(&self) -> Option<&Search> {
        match &self.intent {
            Intent::Search(search) => Some(search),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.intent == Intent::Empty
    }

    #[must_use]
    pub fn suggestion_terms(&self) -> Option<&str> {
        match &self.intent {
            Intent::Search(search) => Some(search.terms()),
            Intent::Navigate(destination) if destination.bare => Some(&self.text),
            _ => None,
        }
    }

    pub(crate) fn ends_with_space(&self) -> bool {
        self.trailing_space
    }
}

impl Destination {
    fn new(url: Url, upgrade: Upgrade) -> Destination {
        Destination {
            url,
            upgrade,
            bare: false,
        }
    }

    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    #[must_use]
    pub fn upgrade(&self) -> Upgrade {
        self.upgrade
    }

    #[must_use]
    pub fn http_fallback(&self) -> Option<Url> {
        if self.upgrade != Upgrade::Assumed {
            return None;
        }
        let mut fallback = self.url.clone();
        fallback.set_scheme("http").ok()?;
        Some(fallback)
    }
}

#[must_use]
pub fn classify(text: &str, engines: &SearchEngines) -> Classified {
    let trailing_space = text.ends_with(char::is_whitespace);
    let text = text.trim();
    let (intent, alternative) = resolve(text, engines);
    Classified {
        text: text.to_owned(),
        intent,
        alternative,
        trailing_space,
    }
}

fn resolve(text: &str, engines: &SearchEngines) -> (Intent, Option<Destination>) {
    if text.is_empty() {
        return (Intent::Empty, None);
    }
    if let Some(forced) = text.strip_prefix('?') {
        let terms = forced.trim_start();
        if terms.is_empty() {
            return (Intent::Empty, None);
        }
        return (search(terms, engines), None);
    }
    if let Some(search) = keyword_search(text, engines) {
        return (Intent::Search(search), None);
    }
    if text.starts_with(['"', '\'', '\u{201C}', '\u{300C}', '\u{300E}']) {
        return (search(text, engines), None);
    }
    if let Some(destination) = file_path(text) {
        return (Intent::Navigate(destination), None);
    }
    let folded = width_folded(text);
    let typed_plainly = matches!(folded, Cow::Borrowed(_));
    match resolve_url(&folded, engines) {
        Resolved::Navigate(destination) => (Intent::Navigate(destination), None),
        Resolved::Search(alternative) => {
            (search(text, engines), alternative.filter(|_| typed_plainly))
        }
    }
}

fn search(terms: &str, engines: &SearchEngines) -> Intent {
    Intent::Search(engines.default_engine().search(terms))
}

fn keyword_search(text: &str, engines: &SearchEngines) -> Option<Search> {
    let (keyword, terms) = text.split_once(char::is_whitespace)?;
    let terms = terms.trim_start();
    if terms.is_empty() {
        return None;
    }
    Some(engines.by_keyword(keyword)?.keyword_search(terms))
}

fn file_path(text: &str) -> Option<Destination> {
    let path = if text.starts_with('/') {
        PathBuf::from(text)
    } else if text == "~" {
        rikka_dirs::home()?.to_owned()
    } else {
        rikka_dirs::home()?.join(text.strip_prefix("~/")?)
    };
    let url = Url::from_file_path(path).ok()?;
    Some(Destination::new(url, Upgrade::NotApplicable))
}

fn resolve_url(text: &str, engines: &SearchEngines) -> Resolved {
    let head = text.split(['/', '?', '#']).next().unwrap_or(text);
    if is_bare_ipv6(head) {
        return schemeless(text);
    }
    if let Some((scheme, rest)) = split_scheme(text) {
        if let Some(resolved) = explicit(text, &scheme, rest, engines) {
            return resolved;
        }
        if !starts_with_port(rest) && !authority(text).contains('@') {
            return Resolved::Search(None);
        }
    }
    schemeless(text)
}

fn split_scheme(text: &str) -> Option<(String, &str)> {
    let (scheme, rest) = text.split_once(':')?;
    let mut bytes = scheme.bytes();
    let valid = bytes.next()?.is_ascii_alphabetic()
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'));
    valid.then(|| (scheme.to_ascii_lowercase(), rest))
}

fn starts_with_port(rest: &str) -> bool {
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    digits > 0 && (digits == rest.len() || rest[digits..].starts_with(['/', '?', '#']))
}

fn authority(text: &str) -> &str {
    let after_scheme = text.split_once("//").map_or(text, |(_, rest)| rest);
    after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme)
}

fn explicit(text: &str, scheme: &str, rest: &str, engines: &SearchEngines) -> Option<Resolved> {
    Some(match scheme {
        "javascript" => Resolved::Search(None),
        "http" | "https" => web(text),
        "file" | "data" | "blob" => plain(text),
        "about" => about(rest),
        "view-source" => view_source(rest, engines),
        "rikka" => internal(rest),
        other if EXTERNAL_SCHEMES.contains(&other) => plain(text),
        _ => return None,
    })
}

fn web(text: &str) -> Resolved {
    let Ok(url) = Url::parse(text) else {
        return Resolved::Search(None);
    };
    let Some(host) = url.host() else {
        return Resolved::Search(None);
    };
    let upgrade = if url.scheme() == "https" {
        Upgrade::NotApplicable
    } else if host::class(&host).exempt_from_upgrade() {
        Upgrade::Exempt
    } else {
        Upgrade::Available
    };
    Resolved::Navigate(Destination::new(url, upgrade))
}

fn plain(text: &str) -> Resolved {
    match Url::parse(text) {
        Ok(url) => Resolved::Navigate(Destination::new(url, Upgrade::NotApplicable)),
        Err(_) => Resolved::Search(None),
    }
}

fn about(rest: &str) -> Resolved {
    let page = rest.trim().to_ascii_lowercase();
    if page.is_empty() {
        return plain("about:blank");
    }
    let name = page.split(['/', '?', '#']).next().unwrap_or_default();
    if INTERNAL_PAGES.contains(&name) {
        return internal(&page);
    }
    plain(&format!("about:{page}"))
}

fn internal(rest: &str) -> Resolved {
    let rest = rest.trim_start_matches('/');
    let split = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (page, tail) = rest.split_at(split);
    if page.is_empty() {
        return Resolved::Search(None);
    }
    plain(&format!("rikka://{}{tail}", page.to_ascii_lowercase()))
}

fn view_source(rest: &str, engines: &SearchEngines) -> Resolved {
    let inner = rest.trim();
    let destination = match file_path(inner) {
        Some(destination) => destination,
        None => match resolve_url(&width_folded(inner), engines) {
            Resolved::Navigate(destination) => destination,
            Resolved::Search(_) => return Resolved::Search(None),
        },
    };
    if !matches!(destination.url.scheme(), "http" | "https" | "file") {
        return Resolved::Search(None);
    }
    plain(&format!("view-source:{}", destination.url))
}

fn schemeless(text: &str) -> Resolved {
    let end = text.find(['/', '?', '#', '\\']).unwrap_or(text.len());
    let (authority, rest) = text.split_at(end);
    if authority.is_empty() || authority.contains(char::is_whitespace) {
        return Resolved::Search(None);
    }
    let (credentials, host_port) = match authority.rsplit_once('@') {
        Some((credentials, host_port)) => (Some(credentials), host_port),
        None => (None, authority),
    };
    let password = credentials.is_some_and(|credentials| credentials.contains(':'));
    if host_port.is_empty() || (credentials.is_some() && !password) {
        return Resolved::Search(None);
    }
    if credentials.is_none() && is_bare_ipv6(host_port) {
        return navigate(&format!("http://[{host_port}]{rest}"), Upgrade::Exempt);
    }
    let Some((host_text, port)) = split_port(host_port) else {
        return Resolved::Search(None);
    };
    let Ok(host) = Host::parse(host_text) else {
        return Resolved::Search(None);
    };
    let domain = match host {
        Host::Ipv4(_) if host_text.parse::<Ipv4Addr>().is_err() => return Resolved::Search(None),
        Host::Ipv4(_) | Host::Ipv6(_) => {
            return navigate(&format!("http://{text}"), Upgrade::Exempt);
        }
        Host::Domain(domain) => domain,
    };
    if !host::is_plausible_host(&domain) {
        return Resolved::Search(None);
    }
    match host::domain_class(&domain) {
        HostClass::Local => navigate(&format!("http://{text}"), Upgrade::Exempt),
        HostClass::Public => {
            let resolved = navigate(&format!("https://{text}"), Upgrade::Assumed);
            match resolved {
                Resolved::Navigate(mut destination) => {
                    destination.bare = port.is_none() && rest.is_empty() && credentials.is_none();
                    Resolved::Navigate(destination)
                }
                searched => searched,
            }
        }
        HostClass::NonUnique | HostClass::Address => {
            if port.is_some() || password {
                return navigate(&format!("http://{text}"), Upgrade::Exempt);
            }
            let intranet = host_text.is_ascii()
                && host::is_strict_label_host(&domain)
                && !host::is_public_suffix(&domain)
                && (rest.is_empty() || rest.starts_with('/'))
                && !rest.contains(char::is_whitespace);
            if !intranet {
                return Resolved::Search(None);
            }
            match navigate(&format!("http://{text}"), Upgrade::Exempt) {
                Resolved::Navigate(destination) => Resolved::Search(Some(destination)),
                searched => searched,
            }
        }
    }
}

fn is_bare_ipv6(text: &str) -> bool {
    text.bytes().any(|byte| byte.is_ascii_digit()) && text.parse::<Ipv6Addr>().is_ok()
}

fn split_port(host_port: &str) -> Option<(&str, Option<&str>)> {
    if host_port.starts_with('[') {
        let close = host_port.find(']')?;
        let (host, after) = host_port.split_at(close + 1);
        return match after.strip_prefix(':') {
            None if after.is_empty() => Some((host, None)),
            Some(port) if valid_port(port) => Some((host, Some(port))),
            _ => None,
        };
    }
    match host_port.rsplit_once(':') {
        None => Some((host_port, None)),
        Some((host, port)) if valid_port(port) => Some((host, Some(port))),
        Some(_) => None,
    }
}

fn valid_port(port: &str) -> bool {
    !port.is_empty()
        && port.bytes().all(|byte| byte.is_ascii_digit())
        && port.parse::<u16>().is_ok()
}

fn navigate(text: &str, upgrade: Upgrade) -> Resolved {
    match Url::parse(text) {
        Ok(url) if url.host().is_some() => Resolved::Navigate(Destination::new(url, upgrade)),
        _ => Resolved::Search(None),
    }
}
