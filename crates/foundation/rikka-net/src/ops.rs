use std::path::Path;
use std::sync::OnceLock;

use bytes::Bytes;

use crate::client::Client;
use crate::download::DownloadBuilder;
use crate::error::Result;
use crate::into_url::IntoUrl;
use crate::response::Response;

pub(crate) fn shared() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(Client::new)
}

pub async fn get(url: impl IntoUrl) -> Result<Response> {
    shared().get(url).send().await
}

pub async fn get_text(url: impl IntoUrl) -> Result<String> {
    shared().get_text(url).await
}

pub async fn get_bytes(url: impl IntoUrl) -> Result<Bytes> {
    shared().get_bytes(url).await
}

#[must_use]
pub fn download_with(url: impl IntoUrl, destination: impl AsRef<Path>) -> DownloadBuilder {
    shared().download(url, destination)
}
