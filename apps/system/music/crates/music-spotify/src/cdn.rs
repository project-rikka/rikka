use std::time::SystemTime;

use music_core::{Error, Result};
use music_spotify_api::{SpClient, USER_AGENT};
use music_spotify_wire::FileId;
use music_stream::{Fetcher, Plan, Spool, answer};
use rikka_net::header;

use crate::failure;

const TRIES: u32 = 3;
const FIRST_BYTE: &str = "bytes=0-0";

pub(crate) struct Cdn {
    fetcher: Fetcher,
}

impl Cdn {
    pub(crate) fn new() -> Cdn {
        Cdn {
            fetcher: Fetcher::new(USER_AGENT).tries(TRIES),
        }
    }

    pub(crate) async fn plan(&self, addresses: &[String]) -> Result<Plan> {
        let mut failure = Error::Missing("a Spotify stream address".into());
        for address in addresses {
            match self.length(address).await {
                Ok(length) => return Ok(Plan::file(address.as_str(), Some(length))),
                Err(error) => {
                    tracing::debug!("Spotify stream host failed, trying the next: {error}");
                    failure = error;
                }
            }
        }
        Err(failure)
    }

    pub(crate) async fn download(&self, plan: Plan, spool: &mut Spool) -> Result<()> {
        self.fetcher.fetch(plan, spool).await
    }

    async fn length(&self, address: &str) -> Result<u64> {
        let request = self
            .fetcher
            .client()
            .get(address)
            .header(header::RANGE, FIRST_BYTE);
        answer(request)
            .await?
            .headers()
            .get(header::CONTENT_RANGE)
            .and_then(|range| range.to_str().ok())
            .and_then(|range| range.rsplit_once('/'))
            .and_then(|(_, length)| length.parse().ok())
            .filter(|length| *length > 0)
            .ok_or_else(|| Error::Unreadable("the Spotify stream host reported no length".into()))
    }
}

pub(crate) async fn addresses(api: &SpClient, file: FileId) -> Result<Vec<String>> {
    let resolved = api.audio_urls(file).await.map_err(failure::api)?;
    if resolved.is_empty() {
        return Err(Error::Missing(format!(
            "a Spotify stream address for {file}"
        )));
    }
    let now = SystemTime::now();
    let live: Vec<String> = resolved
        .into_iter()
        .filter(|address| address.is_live(now))
        .map(|address| address.url)
        .collect();
    if live.is_empty() {
        return Err(Error::Unreachable(format!(
            "every Spotify stream address for {file} has expired"
        )));
    }
    Ok(live)
}
