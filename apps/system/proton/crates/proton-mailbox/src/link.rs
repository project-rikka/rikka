use percent_encoding::percent_decode_str;
use url::Url;

use crate::draft::Draft;

#[derive(Clone, Debug)]
pub enum Link {
    Web(Url),
    Compose(Draft),
}

impl Link {
    #[must_use]
    pub fn parse(target: &str) -> Option<Link> {
        let url = Url::parse(target.trim()).ok()?;
        match url.scheme() {
            "http" | "https" => Some(Link::Web(url)),
            "mailto" => Some(Link::Compose(composed(&url))),
            _ => None,
        }
    }
}

fn composed(url: &Url) -> Draft {
    let mut to = vec![decoded(url.path())];
    let mut cc = Vec::new();
    let mut bcc = Vec::new();
    let mut draft = Draft::blank(None);
    for field in url.query().unwrap_or_default().split('&') {
        let (name, value) = field.split_once('=').unwrap_or((field, ""));
        let value = decoded(value);
        match name.to_ascii_lowercase().as_str() {
            "to" => to.push(value),
            "cc" => cc.push(value),
            "bcc" => bcc.push(value),
            "subject" => draft.subject = value,
            "body" => draft.body = value,
            _ => {}
        }
    }
    draft.to = listed(to);
    draft.cc = listed(cc);
    draft.bcc = listed(bcc);
    draft
}

fn decoded(text: &str) -> String {
    percent_decode_str(text).decode_utf8_lossy().into_owned()
}

fn listed(addresses: Vec<String>) -> String {
    addresses
        .iter()
        .flat_map(|list| list.split(','))
        .map(str::trim)
        .filter(|address| !address.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}
