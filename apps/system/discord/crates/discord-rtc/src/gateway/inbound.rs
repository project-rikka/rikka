use std::time::Duration;

use discord_core::UserId;
use discord_rtp::KEY_LEN;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::dave::Input;
use crate::error::{Error, Result};
use crate::gateway::opcode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Received {
    pub(crate) seq: Option<i64>,
    pub(crate) inbound: Inbound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Inbound {
    Hello { interval: Duration },
    Ready(Ready),
    Described(Description),
    Updated { keyframes: Option<Duration> },
    Speaking { user: UserId, ssrc: u32 },
    Video(Video),
    Wanted(Wants),
    Ack { nonce: u64 },
    Resumed,
    Joined(Vec<UserId>),
    Left(UserId),
    Dave(Input),
    Ignored(u8),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct Ready {
    pub(crate) ssrc: u32,
    pub(crate) ip: String,
    pub(crate) port: u16,
    pub(crate) modes: Vec<String>,
    #[serde(default)]
    pub(crate) streams: Vec<Assigned>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub(crate) struct Assigned {
    pub(crate) ssrc: u32,
    #[serde(default)]
    pub(crate) rtx_ssrc: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Video {
    pub(crate) user: UserId,
    pub(crate) audio: u32,
    pub(crate) streams: Vec<Assigned>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Wants {
    pub(crate) any: Option<u8>,
    pub(crate) sources: Vec<(u32, u8)>,
}

impl Wants {
    pub(crate) fn quality_for(&self, ssrc: u32) -> Option<u8> {
        self.sources
            .iter()
            .find(|(source, _)| *source == ssrc)
            .map(|(_, quality)| *quality)
            .or(self.any)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Description {
    pub(crate) mode: String,
    pub(crate) key: [u8; KEY_LEN],
    pub(crate) dave: u16,
    pub(crate) video_codec: Option<String>,
    pub(crate) keyframes: Option<Duration>,
}

impl std::fmt::Debug for Description {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Description")
            .field("mode", &self.mode)
            .field("dave", &self.dave)
            .field("video_codec", &self.video_codec)
            .field("keyframes", &self.keyframes)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct Envelope {
    op: u8,
    #[serde(default)]
    d: Value,
    #[serde(default)]
    seq: Option<i64>,
}

#[derive(Deserialize)]
struct Hello {
    heartbeat_interval: f64,
}

#[derive(Deserialize)]
struct Described {
    mode: String,
    secret_key: Vec<u8>,
    #[serde(default)]
    dave_protocol_version: u16,
    #[serde(default)]
    video_codec: Option<String>,
    #[serde(default)]
    keyframe_interval: Option<u64>,
}

#[derive(Deserialize)]
struct SessionUpdate {
    #[serde(default)]
    keyframe_interval: Option<u64>,
}

fn keyframes(milliseconds: Option<u64>) -> Option<Duration> {
    milliseconds
        .filter(|milliseconds| *milliseconds > 0)
        .map(Duration::from_millis)
}

#[derive(Deserialize)]
struct Speaking {
    user_id: UserId,
    ssrc: u32,
}

#[derive(Deserialize)]
struct Announced {
    user_id: UserId,
    #[serde(default)]
    audio_ssrc: u32,
    #[serde(default)]
    video_ssrc: u32,
    #[serde(default)]
    streams: Vec<AnnouncedStream>,
}

#[derive(Deserialize)]
struct AnnouncedStream {
    #[serde(default)]
    ssrc: u32,
    #[serde(default)]
    rtx_ssrc: Option<u32>,
    #[serde(default = "active")]
    active: bool,
}

fn active() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Ack {
    Nonce { t: u64 },
    Bare(u64),
}

#[derive(Deserialize)]
struct Joined {
    user_ids: Vec<UserId>,
}

#[derive(Deserialize)]
struct Left {
    user_id: UserId,
}

#[derive(Deserialize)]
struct Transition {
    transition_id: u16,
    #[serde(default)]
    protocol_version: u16,
}

#[derive(Deserialize)]
struct Epoch {
    epoch: u64,
    protocol_version: u16,
}

pub(crate) fn text(json: &str) -> Result<Received> {
    let envelope: Envelope = serde_json::from_str(json).map_err(Error::unreadable)?;
    let data = envelope.d;
    let inbound = match envelope.op {
        opcode::HELLO => {
            let hello: Hello = parse(data)?;
            let interval = Duration::try_from_secs_f64(hello.heartbeat_interval / 1000.0)
                .ok()
                .filter(|interval| !interval.is_zero())
                .ok_or_else(|| Error::Unreadable("the heartbeat interval".to_owned()))?;
            Inbound::Hello { interval }
        }
        opcode::READY => Inbound::Ready(parse(data)?),
        opcode::SESSION_DESCRIPTION => {
            let described: Described = parse(data)?;
            let key = described
                .secret_key
                .try_into()
                .map_err(|_| Error::Unreadable("the transport key length".to_owned()))?;
            Inbound::Described(Description {
                mode: described.mode,
                key,
                dave: described.dave_protocol_version,
                video_codec: described.video_codec,
                keyframes: keyframes(described.keyframe_interval),
            })
        }
        opcode::SESSION_UPDATE => Inbound::Updated {
            keyframes: keyframes(parse::<SessionUpdate>(data)?.keyframe_interval),
        },
        opcode::SPEAKING => {
            let speaking: Speaking = parse(data)?;
            Inbound::Speaking {
                user: speaking.user_id,
                ssrc: speaking.ssrc,
            }
        }
        opcode::VIDEO => {
            let announced: Announced = parse(data)?;
            let mut streams: Vec<Assigned> = announced
                .streams
                .iter()
                .filter(|stream| stream.active && stream.ssrc != 0)
                .map(|stream| Assigned {
                    ssrc: stream.ssrc,
                    rtx_ssrc: stream.rtx_ssrc.filter(|ssrc| *ssrc != 0),
                })
                .collect();
            if streams.is_empty() && announced.video_ssrc != 0 && announced.streams.is_empty() {
                streams.push(Assigned {
                    ssrc: announced.video_ssrc,
                    rtx_ssrc: None,
                });
            }
            Inbound::Video(Video {
                user: announced.user_id,
                audio: announced.audio_ssrc,
                streams,
            })
        }
        opcode::MEDIA_SINK_WANTS => Inbound::Wanted(wants(&data)),
        opcode::HEARTBEAT_ACK => Inbound::Ack {
            nonce: match parse(data)? {
                Ack::Nonce { t } | Ack::Bare(t) => t,
            },
        },
        opcode::RESUMED => Inbound::Resumed,
        opcode::CLIENTS_CONNECT => Inbound::Joined(parse::<Joined>(data)?.user_ids),
        opcode::CLIENT_DISCONNECT => Inbound::Left(parse::<Left>(data)?.user_id),
        opcode::DAVE_PREPARE_TRANSITION => {
            let transition: Transition = parse(data)?;
            Inbound::Dave(Input::PrepareTransition {
                transition: transition.transition_id,
                version: transition.protocol_version,
            })
        }
        opcode::DAVE_EXECUTE_TRANSITION => {
            let transition: Transition = parse(data)?;
            Inbound::Dave(Input::ExecuteTransition {
                transition: transition.transition_id,
            })
        }
        opcode::DAVE_PREPARE_EPOCH => {
            let epoch: Epoch = parse(data)?;
            Inbound::Dave(Input::PrepareEpoch {
                version: epoch.protocol_version,
                epoch: epoch.epoch,
            })
        }
        other => Inbound::Ignored(other),
    };
    Ok(Received {
        seq: envelope.seq,
        inbound,
    })
}

pub(crate) fn binary(frame: &[u8]) -> Result<Received> {
    let [high, low, op, payload @ ..] = frame else {
        return Err(Error::Unreadable(
            "a binary message without header".to_owned(),
        ));
    };
    let inbound = match *op {
        opcode::DAVE_EXTERNAL_SENDER => Inbound::Dave(Input::ExternalSender(payload.to_vec())),
        opcode::DAVE_PROPOSALS => {
            let [kind, proposals @ ..] = payload else {
                return Err(truncated(*op));
            };
            let revoke = match kind {
                0 => false,
                1 => true,
                _ => return Err(Error::Unreadable(format!("proposal operation {kind}"))),
            };
            Inbound::Dave(Input::Proposals {
                revoke,
                proposals: proposals.to_vec(),
            })
        }
        opcode::DAVE_ANNOUNCE_COMMIT => {
            let (transition, commit) = transition(*op, payload)?;
            Inbound::Dave(Input::Commit {
                transition,
                commit: commit.to_vec(),
            })
        }
        opcode::DAVE_WELCOME => {
            let (transition, welcome) = transition(*op, payload)?;
            Inbound::Dave(Input::Welcome {
                transition,
                welcome: welcome.to_vec(),
            })
        }
        other => Inbound::Ignored(other),
    };
    Ok(Received {
        seq: Some(i64::from(u16::from_be_bytes([*high, *low]))),
        inbound,
    })
}

fn wants(data: &Value) -> Wants {
    let mut wants = Wants::default();
    let Some(entries) = data.as_object() else {
        return wants;
    };
    for (key, value) in entries {
        let Some(quality) = value.as_u64().map(|quality| quality.min(100) as u8) else {
            continue;
        };
        if key == "any" {
            wants.any = Some(quality);
        } else if let Ok(ssrc) = key.parse() {
            wants.sources.push((ssrc, quality));
        }
    }
    wants.sources.sort_unstable();
    wants
}

fn transition(op: u8, payload: &[u8]) -> Result<(u16, &[u8])> {
    let [high, low, rest @ ..] = payload else {
        return Err(truncated(op));
    };
    Ok((u16::from_be_bytes([*high, *low]), rest))
}

fn truncated(op: u8) -> Error {
    Error::Unreadable(format!("a truncated binary message {op}"))
}

fn parse<T: DeserializeOwned>(data: Value) -> Result<T> {
    serde_json::from_value(data).map_err(Error::unreadable)
}
