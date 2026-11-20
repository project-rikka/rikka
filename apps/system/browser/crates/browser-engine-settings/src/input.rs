use rikka_web::Endpoint;
use url::Url;

use crate::error::InputError;

const SCRIPT_SCHEMES: [&str; 3] = ["http", "https", "file"];

pub fn parse_resolver(text: &str) -> Result<String, InputError> {
    let url = Url::parse(text.trim()).map_err(|_| InputError::MalformedUrl)?;
    if url.scheme() != "https" {
        return Err(InputError::NotHttps);
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(InputError::MalformedUrl);
    }
    Ok(url.into())
}

pub fn parse_script(text: &str) -> Result<String, InputError> {
    let url = Url::parse(text.trim()).map_err(|_| InputError::MalformedUrl)?;
    if !SCRIPT_SCHEMES.contains(&url.scheme()) {
        return Err(InputError::UnsupportedScheme);
    }
    Ok(url.into())
}

pub fn parse_endpoint(text: &str) -> Result<Option<Endpoint>, InputError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let (host, port) = text.rsplit_once(':').ok_or(InputError::InvalidPort)?;
    let port: u16 = port.parse().map_err(|_| InputError::InvalidPort)?;
    if port == 0 {
        return Err(InputError::InvalidPort);
    }
    let host = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    let invalid = |character: char| character.is_whitespace() || "/?#@".contains(character);
    if host.is_empty() || host.contains(invalid) {
        return Err(InputError::MissingHost);
    }
    Ok(Some(Endpoint::new(host, port)))
}

pub fn parse_bypass(text: &str) -> Vec<String> {
    text.split(|character: char| character == ',' || character.is_whitespace())
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect()
}
