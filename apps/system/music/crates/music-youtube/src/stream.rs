use music_core::{Error, Quality, Result};
use music_stream::Plan;
use rand::Rng;
use rand::distr::Alphanumeric;
use rikka_net::Url;
use serde_json::{Value, json};

use crate::clients::Client;
use crate::innertube::{As, Innertube};
use crate::json::Walk;
use crate::solver::Solving;

const NONCE: usize = 16;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Stream {
    pub(crate) url: String,
    pub(crate) agent: &'static str,
    pub(crate) mime: String,
    pub(crate) bitrate: u64,
    pub(crate) size: Option<u64>,
}

pub(crate) async fn resolve(
    tube: &Innertube,
    solving: &Solving,
    video: &str,
    quality: Quality,
) -> Result<Stream> {
    match guest(tube, video, quality).await {
        Ok(resolved) => Ok(resolved),
        Err(refused) => {
            tracing::debug!("guest stream of {video} refused, retrying signed in: {refused}");
            signed(tube, solving, video, quality).await
        }
    }
}

async fn guest(tube: &Innertube, video: &str, quality: Quality) -> Result<Stream> {
    let visitor = tube.visitor().await;
    let mut answer = tube
        .post(
            "player",
            Client::VisionOs,
            request(video, None),
            As::Guest(visitor.as_deref()),
        )
        .await?;
    if status(&answer) == "LOGIN_REQUIRED"
        && let Some(issued) = answer
            .text_at(&["responseContext", "visitorData"])
            .map(str::to_owned)
    {
        tube.adopt_visitor(issued.clone());
        answer = tube
            .post(
                "player",
                Client::VisionOs,
                request(video, None),
                As::Guest(Some(&issued)),
            )
            .await?;
    }
    let formats = playable(&answer, video)?;
    best(
        formats
            .iter()
            .filter_map(|format| direct(format, Client::VisionOs)),
        quality,
    )
    .ok_or_else(|| Error::Missing(format!("a direct audio stream of {video}")))
}

async fn signed(
    tube: &Innertube,
    solving: &Solving,
    video: &str,
    quality: Quality,
) -> Result<Stream> {
    let solver = solving.solver().await?;
    let answer = tube
        .post(
            "player",
            Client::Music,
            request(video, Some(solver.sts())),
            As::Account,
        )
        .await?;
    let formats = playable(&answer, video)?;
    let Some(chosen) = best_ciphered(formats.iter().filter_map(ciphered), quality) else {
        return Err(Error::Missing(format!("an audio stream of {video}")));
    };
    let mut url =
        Url::parse(&chosen.stream.url).map_err(|error| Error::Unreadable(error.to_string()))?;
    let throttle = param(&url, "n");
    let solved = match solver
        .solve(
            chosen
                .signature
                .as_ref()
                .map(|(_, signature)| signature.as_str()),
            throttle.as_deref(),
        )
        .await
    {
        Ok(solved) => solved,
        Err(error) => {
            solving.outdated(&solver).await;
            return Err(error);
        }
    };
    let mut changes = Vec::new();
    if let (Some((into, _)), Some(signature)) = (&chosen.signature, &solved.signature) {
        changes.push((into.clone(), signature.clone()));
    }
    if let Some(throttle) = solved.throttle {
        changes.push(("n".to_owned(), throttle));
    }
    set_params(&mut url, &changes);
    Ok(Stream {
        url: url.into(),
        ..chosen.stream
    })
}

fn request(video: &str, sts: Option<u32>) -> Value {
    let nonce: String = rand::rng()
        .sample_iter(Alphanumeric)
        .take(NONCE)
        .map(char::from)
        .collect();
    let mut playback = json!({ "html5Preference": "HTML5_PREF_WANTS" });
    if let Some(sts) = sts {
        playback["signatureTimestamp"] = json!(sts);
    }
    json!({
        "videoId": video,
        "contentCheckOk": true,
        "racyCheckOk": true,
        "cpn": nonce,
        "playbackContext": { "contentPlaybackContext": playback },
    })
}

fn status(answer: &Value) -> &str {
    answer
        .text_at(&["playabilityStatus", "status"])
        .unwrap_or("UNKNOWN")
}

fn playable<'a>(answer: &'a Value, video: &str) -> Result<&'a [Value]> {
    let status = status(answer);
    if status != "OK" {
        let reason = answer
            .text_at(&["playabilityStatus", "reason"])
            .unwrap_or("no reason given");
        return Err(match status {
            "LOGIN_REQUIRED" => Error::Refused(format!("{video}: {reason}")),
            _ => Error::Missing(format!("{video} ({status}: {reason})")),
        });
    }
    let formats = answer.list(&["streamingData", "adaptiveFormats"]);
    match formats.is_empty() {
        true => Err(Error::Missing(format!("the audio of {video}"))),
        false => Ok(formats),
    }
}

fn described(format: &Value, url: String, agent: &'static str) -> Option<Stream> {
    let mime = format.text_at(&["mimeType"])?;
    if !mime.starts_with("audio/") {
        return None;
    }
    Some(Stream {
        url,
        agent,
        mime: mime.to_owned(),
        bitrate: format.at(&["bitrate"]).and_then(Value::as_u64).unwrap_or(0),
        size: format
            .text_at(&["contentLength"])
            .and_then(|text| text.parse::<u64>().ok()),
    })
}

fn direct(format: &Value, client: Client) -> Option<Stream> {
    let url = format.text_at(&["url"])?;
    described(format, url.to_owned(), client.agent())
}

struct Ciphered {
    stream: Stream,
    signature: Option<(String, String)>,
}

fn ciphered(format: &Value) -> Option<Ciphered> {
    let (url, signature) = match format.text_at(&["signatureCipher"]) {
        Some(cipher) => {
            let cipher_url = Url::parse(&format!("https://cipher/?{cipher}")).ok()?;
            let into = param(&cipher_url, "sp").unwrap_or_else(|| "signature".to_owned());
            (
                param(&cipher_url, "url")?,
                Some((into, param(&cipher_url, "s")?)),
            )
        }
        None => (format.text_at(&["url"])?.to_owned(), None),
    };
    Some(Ciphered {
        stream: described(format, url, Client::Music.agent())?,
        signature,
    })
}

fn ceiling(quality: Quality) -> u64 {
    match quality {
        Quality::Low => 96_000,
        Quality::Standard => 192_000,
        Quality::Highest => u64::MAX,
    }
}

fn preference(stream: &Stream, quality: Quality) -> (bool, bool, u64) {
    let within = stream.bitrate <= ceiling(quality);
    let closeness = match within {
        true => stream.bitrate,
        false => u64::MAX - stream.bitrate,
    };
    (stream.mime.starts_with("audio/mp4"), within, closeness)
}

fn best(streams: impl Iterator<Item = Stream>, quality: Quality) -> Option<Stream> {
    streams.max_by_key(|stream| preference(stream, quality))
}

fn best_ciphered(streams: impl Iterator<Item = Ciphered>, quality: Quality) -> Option<Ciphered> {
    streams.max_by_key(|ciphered| preference(&ciphered.stream, quality))
}

fn param(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.into_owned())
}

fn set_params(url: &mut Url, changes: &[(String, String)]) {
    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect();
    for (key, value) in changes {
        match pairs.iter_mut().find(|(name, _)| name == key) {
            Some(pair) => pair.1.clone_from(value),
            None => pairs.push((key.clone(), value.clone())),
        }
    }
    url.query_pairs_mut().clear().extend_pairs(pairs);
}

pub(crate) fn plan(stream: &Stream) -> Plan {
    Plan::file(stream.url.as_str(), stream.size)
        .header("User-Agent", stream.agent)
        .header("Accept-Encoding", "identity")
        .header("Origin", "https://www.youtube.com")
        .header("Referer", "https://www.youtube.com/")
}
