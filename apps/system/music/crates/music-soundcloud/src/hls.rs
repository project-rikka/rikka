use music_core::{Error, Result};
use rikka_net::Url;

const MAP: &str = "#EXT-X-MAP:";
const KEY: &str = "#EXT-X-KEY:";
const INF: &str = "#EXTINF:";
const VARIANT: &str = "#EXT-X-STREAM-INF";
const BYTE_RANGE: &str = "#EXT-X-BYTERANGE";

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Listing {
    pub(crate) init: Option<String>,
    pub(crate) segments: Vec<String>,
    pub(crate) seconds: f64,
}

pub(crate) fn listing(playlist: &str, base: &str) -> Result<Listing> {
    let base = Url::parse(base).map_err(|error| Error::Unreadable(error.to_string()))?;
    let resolve = |reference: &str| {
        base.join(reference)
            .map(String::from)
            .map_err(|error| Error::Unreadable(format!("a segment address: {error}")))
    };
    let mut listed = Listing {
        init: None,
        segments: Vec::new(),
        seconds: 0.0,
    };
    for line in playlist
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if let Some(attributes) = line.strip_prefix(MAP) {
            let reference = attribute(attributes, "URI")
                .ok_or_else(|| Error::Unreadable("a segment map has no address".into()))?;
            listed.init = Some(resolve(reference)?);
        } else if let Some(attributes) = line.strip_prefix(KEY) {
            if attribute(attributes, "METHOD").is_some_and(|method| method != "NONE") {
                return Err(Error::NotEntitled("the stream is encrypted".into()));
            }
        } else if let Some(information) = line.strip_prefix(INF) {
            let seconds = information.split(',').next().unwrap_or_default().trim();
            listed.seconds += seconds.parse::<f64>().unwrap_or(0.0);
        } else if line.starts_with(VARIANT) {
            return Err(Error::Unreadable(
                "a variant list instead of a stream".into(),
            ));
        } else if line.starts_with(BYTE_RANGE) {
            return Err(Error::Unreadable(
                "a stream of byte ranges within one file".into(),
            ));
        } else if !line.starts_with('#') {
            listed.segments.push(resolve(line)?);
        }
    }
    match listed.segments.is_empty() {
        true => Err(Error::Unreadable("the stream has no segments".into())),
        false => Ok(listed),
    }
}

fn attribute<'a>(attributes: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = attributes;
    while !rest.is_empty() {
        let (key, after) = rest.split_once('=')?;
        let (value, remaining) = match after.strip_prefix('"') {
            Some(quoted) => {
                let (value, tail) = quoted.split_once('"')?;
                (value, tail.strip_prefix(',').unwrap_or(tail))
            }
            None => after.split_once(',').unwrap_or((after, "")),
        };
        if key.trim() == name {
            return Some(value);
        }
        rest = remaining;
    }
    None
}
