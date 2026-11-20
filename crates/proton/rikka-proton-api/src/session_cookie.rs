use http::{HeaderMap, HeaderValue, header};
use jiff::Timestamp;

const NAME: &str = "Session-Id";
const MAX_AGE: &str = "max-age";
const EXPIRES: &str = "expires";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CookieUpdate {
    Set(String),
    Clear,
}

pub(crate) fn latest_update(headers: &HeaderMap) -> Option<CookieUpdate> {
    headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(parse_set_cookie)
        .next_back()
}

pub(crate) fn cookie_header(value: &str) -> Option<HeaderValue> {
    let mut header = HeaderValue::from_str(&format!("{NAME}={value}")).ok()?;
    header.set_sensitive(true);
    Some(header)
}

fn parse_set_cookie(line: &str) -> Option<CookieUpdate> {
    let mut attributes = line.split(';');
    let (name, value) = attributes.next()?.split_once('=')?;
    if name.trim() != NAME {
        return None;
    }

    let value = value.trim().trim_matches('"');
    if value.is_empty() || is_expired(attributes) {
        return Some(CookieUpdate::Clear);
    }
    Some(CookieUpdate::Set(value.to_owned()))
}

fn is_expired<'a>(attributes: impl Iterator<Item = &'a str>) -> bool {
    let mut max_age = None;
    let mut expires = None;
    for attribute in attributes {
        let Some((key, value)) = attribute.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if key.eq_ignore_ascii_case(MAX_AGE) {
            max_age = value.parse::<i64>().ok().or(max_age);
        } else if key.eq_ignore_ascii_case(EXPIRES) {
            expires = jiff::fmt::rfc2822::parse(value)
                .ok()
                .map(|date| date.timestamp())
                .or(expires);
        }
    }
    match max_age {
        Some(seconds) => seconds <= 0,
        None => expires.is_some_and(|expiry| expiry <= Timestamp::now()),
    }
}
