use std::sync::Arc;

use music_core::{Error, Id, Quality, Result, async_trait};
use music_qobuz_keys::{Vault, content_key};
use music_stream::{Origin, Spool, Stored};
use rikka_audio_decode::StreamReader;
use rikka_hifi::{Gains, Loudness};
use serde_json::Value;

use crate::api::Api;
use crate::cdn::Cdn;
use crate::sealed::{self, Decrypted};
use crate::segments;

const MP3: u32 = 5;
const CD: u32 = 6;
const HIGH: u32 = 7;
const HIGHEST: u32 = 27;

pub(crate) struct QobuzOrigin {
    api: Arc<Api>,
    cdn: Cdn,
    vault: Arc<Vault>,
}

#[derive(Debug, PartialEq, Eq)]
struct Located {
    template: String,
    key: String,
}

impl QobuzOrigin {
    pub(crate) fn new(api: Arc<Api>, vault: Arc<Vault>) -> QobuzOrigin {
        QobuzOrigin {
            api,
            cdn: Cdn::new(),
            vault,
        }
    }
}

#[async_trait]
impl Origin for QobuzOrigin {
    async fn fetch(&self, track: &Id, quality: Quality, spool: &mut Spool) -> Result<()> {
        let session = self.api.session().await?;
        for format in formats(quality) {
            let answer = self.api.file(track, *format, &session).await?;
            let Some(located) = located(&answer)? else {
                continue;
            };
            let init = self.cdn.init(&located.template).await?;
            let described = match segments::init(&init) {
                Ok(described) => described,
                Err(refused) => {
                    tracing::debug!("skipping {track} format {format}: {refused}");
                    continue;
                }
            };
            let key = content_key(self.api.secret(), &session.infos, &located.key)?;
            let head = sealed::head(&self.vault.seal(&key)?, &init)?;
            return self
                .cdn
                .segments(head, &located.template, described.lengths.len(), spool)
                .await;
        }
        Err(Error::Missing(format!("a stream of {track}")))
    }

    fn open(&self, stored: Stored) -> Result<Box<dyn StreamReader>> {
        Ok(Box::new(Decrypted::open(stored, &self.vault)?))
    }

    async fn gains(&self, track: &Id) -> Gains {
        match self
            .api
            .get("track/get", &[("track_id", track.to_string())])
            .await
        {
            Ok(answer) => gains(&answer),
            Err(error) => {
                tracing::debug!("no loudness for {track}: {error}");
                Gains::default()
            }
        }
    }
}

fn gains(answer: &Value) -> Gains {
    let info = answer.get("audio_info");
    let number = |key: &str| {
        info.and_then(|info| info.get(key))
            .and_then(Value::as_f64)
            .map(|number| number as f32)
    };
    let Some(gain) = number("replaygain_track_gain") else {
        return Gains::default();
    };
    let loudness = Loudness::new(gain);
    Gains::default().track(match number("replaygain_track_peak") {
        Some(peak) => loudness.peak(peak),
        None => loudness,
    })
}

fn formats(quality: Quality) -> &'static [u32] {
    match quality {
        Quality::Low => &[MP3, CD],
        Quality::Standard => &[CD],
        Quality::Highest => &[HIGHEST, HIGH, CD],
    }
}

fn located(answer: &Value) -> Result<Option<Located>> {
    if answer.get("sample").and_then(Value::as_bool) == Some(true) {
        return Err(Error::Missing("only a preview can be streamed".into()));
    }
    let field = |key: &str| {
        answer
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    Ok(field("url_template")
        .zip(field("key"))
        .map(|(template, key)| Located { template, key }))
}
