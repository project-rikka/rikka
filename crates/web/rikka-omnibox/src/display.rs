use std::ops::Range;

use url::{Host, Url};

use crate::decode::decode_into;
use crate::host;
use crate::idn;

const OPAQUE_LIMIT: usize = 2048;
const ELLIPSIS: char = '\u{2026}';

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlDisplay {
    text: String,
    host: Option<Range<usize>>,
    domain: Option<Range<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Steady,
    Full,
    Matching,
}

impl UrlDisplay {
    #[must_use]
    pub fn steady(url: &Url) -> UrlDisplay {
        build(url, Style::Steady)
    }

    #[must_use]
    pub fn full(url: &Url) -> UrlDisplay {
        build(url, Style::Full)
    }

    pub(crate) fn matching(url: &Url) -> UrlDisplay {
        build(url, Style::Matching)
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn host(&self) -> Option<Range<usize>> {
        self.host.clone()
    }

    #[must_use]
    pub fn domain(&self) -> Option<Range<usize>> {
        self.domain.clone()
    }

    #[must_use]
    pub fn into_text(self) -> String {
        self.text
    }
}

#[must_use]
pub fn display_host(host: &str) -> String {
    idn::display_host(host)
}

#[must_use]
pub fn copy_text(url: &Url, shown: &str, selection: Range<usize>) -> String {
    let Some(selected) = shown.get(selection.clone()) else {
        return String::new();
    };
    let steady = UrlDisplay::steady(url);
    let unedited = shown == steady.text || shown == UrlDisplay::full(url).text;
    if unedited && selection.start == 0 && selection.end == shown.len() {
        return url.as_str().to_owned();
    }
    if shown == steady.text && selection.start == 0 && matches!(url.scheme(), "http" | "https") {
        let www = url
            .host_str()
            .filter(|host| host.starts_with("www.") && !steady.text.starts_with("www."))
            .map_or("", |_| "www.");
        return format!("{}://{www}{selected}", url.scheme());
    }
    selected.to_owned()
}

fn build(url: &Url, style: Style) -> UrlDisplay {
    match url.scheme() {
        "http" | "https" => web(url, style),
        "view-source" => view_source(url, style),
        _ => other(url, style),
    }
}

fn web(url: &Url, style: Style) -> UrlDisplay {
    let mut text = String::with_capacity(url.as_str().len());
    if style == Style::Full {
        text.push_str(url.scheme());
        text.push_str("://");
    }
    let host_start = text.len();
    let mut domain_labels = 0;
    match url.host() {
        Some(Host::Domain(domain)) => {
            let shown = idn::display_host(domain);
            let strip = style == Style::Steady
                && domain.starts_with("www.")
                && host::registrable(&domain[4..]).is_some();
            text.push_str(if strip { &shown[4..] } else { &shown });
            domain_labels = host::registrable(domain).map_or(0, |registrable| {
                registrable
                    .split('.')
                    .filter(|label| !label.is_empty())
                    .count()
            });
        }
        Some(_) => text.push_str(url.host_str().unwrap_or_default()),
        None => {}
    }
    let host = host_start..text.len();
    let domain = if domain_labels > 0 {
        domain_range(&text, host.clone(), domain_labels)
    } else {
        host.clone()
    };
    if let Some(port) = url.port() {
        text.push(':');
        text.push_str(&port.to_string());
    }
    let path = url.path();
    let root_only = path == "/" && url.query().is_none() && url.fragment().is_none();
    if !(root_only && style != Style::Full) {
        decode_into(&mut text, path);
    }
    push_rest(&mut text, url);
    UrlDisplay {
        text,
        host: Some(host),
        domain: Some(domain),
    }
}

fn domain_range(text: &str, host: Range<usize>, labels: usize) -> Range<usize> {
    let shown = text[host.clone()].trim_end_matches('.');
    let mut start = host.start;
    let mut dots = shown.rmatch_indices('.');
    for _ in 0..labels.saturating_sub(1) {
        if dots.next().is_none() {
            return host.start..host.start + shown.len();
        }
    }
    if let Some((index, _)) = dots.next() {
        start = host.start + index + 1;
    }
    start..host.start + shown.len()
}

fn push_rest(text: &mut String, url: &Url) {
    if let Some(query) = url.query() {
        text.push('?');
        decode_into(text, query);
    }
    if let Some(fragment) = url.fragment() {
        text.push('#');
        decode_into(text, fragment);
    }
}

fn view_source(url: &Url, style: Style) -> UrlDisplay {
    let Ok(inner) = Url::parse(url.path()) else {
        return other(url, style);
    };
    let shown = build(&inner, Style::Full);
    let prefix = "view-source:";
    let shift = |range: Range<usize>| range.start + prefix.len()..range.end + prefix.len();
    UrlDisplay {
        text: format!("{prefix}{}", shown.text),
        host: shown.host.map(shift),
        domain: shown.domain.map(shift),
    }
}

fn other(url: &Url, style: Style) -> UrlDisplay {
    let mut text = String::with_capacity(url.as_str().len().min(OPAQUE_LIMIT + 8));
    text.push_str(url.scheme());
    text.push(':');
    if !url.cannot_be_a_base() {
        text.push_str("//");
        text.push_str(url.host_str().unwrap_or_default());
        if let Some(port) = url.port() {
            text.push(':');
            text.push_str(&port.to_string());
        }
    }
    decode_into(&mut text, url.path());
    push_rest(&mut text, url);
    if style != Style::Full && text.len() > OPAQUE_LIMIT {
        let mut cut = OPAQUE_LIMIT;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push(ELLIPSIS);
    }
    UrlDisplay {
        text,
        host: None,
        domain: None,
    }
}
