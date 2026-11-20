use std::cmp::Reverse;

use music_core::{Error, Quality, Result};
use serde_json::Value;

use crate::parse::text;

const MP3_KBPS: u32 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Tier {
    Low,
    Standard,
    High,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Delivery {
    Progressive,
    Segmented,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Codec {
    Aac,
    Mp3,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Transcoding {
    pub(crate) url: String,
    pub(crate) delivery: Delivery,
    codec: Codec,
    tier: Tier,
    kbps: u32,
    snipped: bool,
}

pub(crate) fn choose(track: &Value, quality: Quality) -> Result<Transcoding> {
    let offered: Vec<Transcoding> = track
        .get("media")
        .and_then(|media| media.get("transcodings"))
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .filter_map(transcoding)
        .collect();
    if offered.is_empty() {
        return Err(Error::Missing("a stream this player can decode".into()));
    }
    let whole: Vec<Transcoding> = offered
        .into_iter()
        .filter(|transcoding| !transcoding.snipped)
        .collect();
    let target = match quality {
        Quality::Low => Tier::Low,
        Quality::Standard => Tier::Standard,
        Quality::Highest => Tier::High,
    };
    whole
        .into_iter()
        .min_by_key(|transcoding| {
            (
                transcoding.tier > target,
                (transcoding.tier as u8).abs_diff(target as u8),
                transcoding.codec != Codec::Aac,
                Reverse(transcoding.kbps),
                transcoding.delivery != Delivery::Progressive,
            )
        })
        .ok_or_else(|| Error::NotEntitled("SoundCloud offers only a preview of this track".into()))
}

fn transcoding(value: &Value) -> Option<Transcoding> {
    let format = value.get("format")?;
    let delivery = match text(format, "protocol")? {
        "progressive" => Delivery::Progressive,
        "hls" => Delivery::Segmented,
        _ => return None,
    };
    let essence = text(format, "mime_type")?.split(';').next()?.trim();
    let preset = text(value, "preset").unwrap_or_default();
    let (codec, kbps) = match essence {
        "audio/mpeg" => (Codec::Mp3, MP3_KBPS),
        "audio/mp4" => (Codec::Aac, kbps_of(preset)),
        _ => return None,
    };
    Some(Transcoding {
        url: text(value, "url")?.to_owned(),
        delivery,
        codec,
        tier: match text(value, "quality") {
            Some("lq") => Tier::Low,
            Some("hq") => Tier::High,
            _ => Tier::Standard,
        },
        kbps,
        snipped: value
            .get("snipped")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn kbps_of(preset: &str) -> u32 {
    preset
        .split('_')
        .find_map(|part| part.strip_suffix('k')?.parse().ok())
        .unwrap_or(0)
}
