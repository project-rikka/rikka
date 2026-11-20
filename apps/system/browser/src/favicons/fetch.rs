use std::sync::OnceLock;
use std::time::Duration;

use browser_pages::data_url_payload;
use rikka_image::ColorScheme;
use rikka_net::Client;

use crate::favicons::decoded;
use browser_model::Favicon;

const LIMIT: usize = 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(10);

fn client() -> Option<&'static Client> {
    static CLIENT: OnceLock<Option<Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .cookies(false)
                .timeout(TIMEOUT)
                .build()
                .inspect_err(|error| tracing::warn!(%error, "cannot create favicon client"))
                .ok()
        })
        .as_ref()
}

pub(crate) async fn fetched(url: String, scheme: ColorScheme) -> Option<Favicon> {
    let bytes = match url.starts_with("data:") {
        true => data_url_payload(&url)?,
        false => remote(&url).await?,
    };
    decoded(&bytes, scheme)
}

async fn remote(url: &str) -> Option<Vec<u8>> {
    let response = client()?
        .get(url)
        .send()
        .await
        .inspect_err(|error| tracing::debug!(%error, %url, "failed to fetch favicon"))
        .ok()?;
    if !response.is_success() {
        return None;
    }
    response
        .bytes_limited(LIMIT)
        .await
        .inspect_err(|error| tracing::debug!(%error, %url, "failed to read favicon response"))
        .ok()
        .map(|bytes| bytes.to_vec())
}
