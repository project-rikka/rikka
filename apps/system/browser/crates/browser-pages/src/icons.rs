use base64::Engine;
use percent_encoding::percent_decode_str;
use url::Url;

const WANTED: u32 = 64;
const FALLBACK: &str = "/favicon.ico";
const BASE64: &str = "base64";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconCandidate<'a> {
    pub url: &'a str,
    pub rel: &'a str,
    pub sizes: &'a [String],
    pub mime_type: Option<&'a str>,
}

#[must_use]
pub fn best_icon<'a>(
    page: &str,
    candidates: impl IntoIterator<Item = IconCandidate<'a>>,
    loaded: bool,
) -> Option<String> {
    let chosen = candidates
        .into_iter()
        .filter(|icon| usable(icon.url))
        .min_by_key(fit);
    match chosen {
        Some(icon) => Some(icon.url.to_owned()),
        None => fallback(page).filter(|_| loaded),
    }
}

#[must_use]
pub fn data_url_payload(url: &str) -> Option<Vec<u8>> {
    let (header, payload) = url.strip_prefix("data:")?.split_once(',')?;
    let encoded = header
        .rsplit(';')
        .next()
        .is_some_and(|parameter| parameter.trim().eq_ignore_ascii_case(BASE64));
    match encoded {
        true => {
            let packed: Vec<u8> = percent_decode_str(payload)
                .filter(|byte| !byte.is_ascii_whitespace())
                .collect();
            base64::engine::general_purpose::STANDARD_NO_PAD
                .decode(trimmed(&packed))
                .ok()
        }
        false => Some(percent_decode_str(payload).collect()),
    }
}

fn trimmed(packed: &[u8]) -> &[u8] {
    let padding = packed
        .iter()
        .rev()
        .take_while(|byte| **byte == b'=')
        .count();
    &packed[..packed.len() - padding]
}

fn usable(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://") || url.starts_with("data:image/")
}

fn fit(icon: &IconCandidate<'_>) -> (u32, u32) {
    let touch = u32::from(icon.rel.to_ascii_lowercase().contains("apple-touch"));
    let vector = icon.mime_type.is_some_and(|mime| mime.contains("svg"))
        || icon
            .sizes
            .iter()
            .any(|size| size.eq_ignore_ascii_case("any"));
    let distance = match vector {
        true => 0,
        false => icon
            .sizes
            .iter()
            .filter_map(|size| side(size))
            .map(|side| match side >= WANTED {
                true => side - WANTED,
                false => (WANTED - side) * 4,
            })
            .min()
            .unwrap_or(WANTED * 2),
    };
    (touch, distance)
}

fn side(size: &str) -> Option<u32> {
    let (width, height) = size
        .to_ascii_lowercase()
        .split_once('x')
        .map(|(width, height)| (width.parse::<u32>().ok(), height.parse::<u32>().ok()))?;
    Some(width?.max(height?))
}

fn fallback(page: &str) -> Option<String> {
    let parsed = Url::parse(page).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    parsed.join(FALLBACK).ok().map(String::from)
}
