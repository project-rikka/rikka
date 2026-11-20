use std::time::{Duration, SystemTime, UNIX_EPOCH};

use music_spotify_wire::FileId;
use prost::Message as _;
use rikka_net::{Method, Url};

use crate::error::{Error, Result};
use crate::messages::storage::StorageResolveResponse;
use crate::spclient::{Call, SpClient};

const CDN: i32 = 0;
const EXPIRY_MARGIN: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CdnUrl {
    pub url: String,
    pub expires: Option<SystemTime>,
}

impl CdnUrl {
    #[must_use]
    pub fn parse(url: &str) -> CdnUrl {
        CdnUrl {
            url: url.to_owned(),
            expires: expiry(url).and_then(|expiry| expiry.checked_sub(EXPIRY_MARGIN)),
        }
    }

    #[must_use]
    pub fn is_live(&self, now: SystemTime) -> bool {
        self.expires.is_none_or(|expires| now < expires)
    }
}

impl SpClient {
    pub async fn audio_urls(&self, file: FileId) -> Result<Vec<CdnUrl>> {
        let path = format!("/storage-resolve/files/audio/interactive/{file}");
        let answer = self
            .call(Call {
                method: Method::GET,
                path: &path,
                accept: None,
                body: None,
            })
            .await?;
        let resolved =
            StorageResolveResponse::decode(answer).map_err(Error::decode("audio storage"))?;
        if resolved.result != CDN {
            return Err(Error::Storage(file.to_base16()));
        }
        Ok(resolved
            .cdnurl
            .iter()
            .map(|url| match resolved.fileid.is_empty() {
                true => CdnUrl {
                    url: url.clone(),
                    expires: None,
                },
                false => CdnUrl::parse(url),
            })
            .collect())
    }
}

fn expiry(url: &str) -> Option<SystemTime> {
    const TOKEN_EXPIRY: &str = "exp=";
    let parsed = Url::parse(url).ok()?;
    let value = |name: &str| {
        parsed
            .query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    let stamp = if let Some(verify) = value("verify") {
        verify.split_once('-')?.0.to_owned()
    } else if let Some(token) = value("__token__") {
        let rest = &token[token.find(TOKEN_EXPIRY)? + TOKEN_EXPIRY.len()..];
        rest.split_once('~')
            .map_or(rest, |(stamp, _)| stamp)
            .to_owned()
    } else if let Some(expires) = value("Expires") {
        expires.split_once('~')?.0.to_owned()
    } else {
        parsed.query()?.split('_').next()?.to_owned()
    };
    let seconds: u64 = stamp.parse().ok()?;
    UNIX_EPOCH.checked_add(Duration::from_secs(seconds))
}
