use std::sync::OnceLock;

use anyhow::{Context, Result};
use rikka_net::Bytes;
use rikka_net::blocking::Client;
use serde::de::DeserializeOwned;

fn client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(Client::new)
}

pub(crate) fn text(url: &str) -> Result<String> {
    client()
        .get_text(url)
        .with_context(|| format!("fetching {url}"))
}

pub(crate) fn bytes(url: &str) -> Result<Bytes> {
    client()
        .get_bytes(url)
        .with_context(|| format!("fetching {url}"))
}

pub(crate) fn json<T: DeserializeOwned>(url: &str) -> Result<T> {
    serde_json::from_str(&text(url)?).with_context(|| format!("unexpected document at {url}"))
}
