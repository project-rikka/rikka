use discord_core::VideoShape;
use discord_rtp::{Mode, OPUS_PAYLOAD_TYPE, Payload, SENT_CODEC, STREAM_RID, VIDEO_CODECS};
use serde_json::{Value, json};

use crate::dave::Reply;
use crate::endpoint::{Endpoint, Role};
use crate::gateway::opcode;

const MICROPHONE: u8 = 1;
const SOUNDSHARE: u8 = 2;
const FULL_QUALITY: u8 = 100;

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Outbound {
    Text(String),
    Binary(Vec<u8>),
}

impl std::fmt::Debug for Outbound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Outbound::Text(text) => write!(formatter, "Text({} bytes)", text.len()),
            Outbound::Binary(bytes) => write!(formatter, "Binary({} bytes)", bytes.len()),
        }
    }
}

pub(crate) fn identify(endpoint: &Endpoint, role: Role, max_dave: u16) -> Outbound {
    let mut data = json!({
        "server_id": endpoint.server.to_string(),
        "channel_id": endpoint.channel,
        "user_id": endpoint.user,
        "session_id": endpoint.session,
        "token": endpoint.token,
        "max_dave_protocol_version": max_dave,
    });
    if role.has_video() {
        data["video"] = json!(true);
        data["streams"] = json!([{ "type": "screen", "rid": STREAM_RID, "quality": FULL_QUALITY }]);
    }
    payload(opcode::IDENTIFY, data)
}

pub(crate) fn resume(endpoint: &Endpoint, seq_ack: i64) -> Outbound {
    payload(
        opcode::RESUME,
        json!({
            "server_id": endpoint.server.to_string(),
            "channel_id": endpoint.channel,
            "session_id": endpoint.session,
            "token": endpoint.token,
            "seq_ack": seq_ack,
        }),
    )
}

pub(crate) fn heartbeat(nonce: u64, seq_ack: i64) -> Outbound {
    payload(opcode::HEARTBEAT, json!({ "t": nonce, "seq_ack": seq_ack }))
}

pub(crate) fn select_protocol(address: &str, port: u16, mode: Mode, role: Role) -> Outbound {
    payload(
        opcode::SELECT_PROTOCOL,
        json!({
            "protocol": "udp",
            "data": { "address": address, "port": port, "mode": mode.name() },
            "codecs": offered(role.has_video()),
        }),
    )
}

pub(crate) fn speaking(on: bool, ssrc: u32, role: Role) -> Outbound {
    let flag = match role {
        Role::Broadcast => SOUNDSHARE,
        Role::Voice | Role::Watch => MICROPHONE,
    };
    payload(
        opcode::SPEAKING,
        json!({ "speaking": if on { flag } else { 0 }, "delay": 0, "ssrc": ssrc }),
    )
}

pub(crate) struct Sending {
    pub(crate) audio: u32,
    pub(crate) video: u32,
    pub(crate) repair: u32,
}

pub(crate) fn video(sending: &Sending, shape: Option<VideoShape>) -> Outbound {
    let Some(shape) = shape else {
        return payload(
            opcode::VIDEO,
            json!({
                "audio_ssrc": sending.audio,
                "video_ssrc": 0,
                "rtx_ssrc": 0,
                "streams": [],
            }),
        );
    };
    payload(
        opcode::VIDEO,
        json!({
            "audio_ssrc": sending.audio,
            "video_ssrc": sending.video,
            "rtx_ssrc": sending.repair,
            "streams": [{
                "type": "video",
                "rid": STREAM_RID,
                "ssrc": sending.video,
                "active": true,
                "quality": FULL_QUALITY,
                "rtx_ssrc": sending.repair,
                "max_bitrate": shape.bitrate,
                "max_framerate": shape.frame_rate,
                "max_resolution": { "type": "fixed", "width": shape.width, "height": shape.height },
            }],
        }),
    )
}

pub(crate) fn wants(sources: &[u32]) -> Outbound {
    let mut data = json!({ "any": FULL_QUALITY });
    for source in sources {
        data[source.to_string()] = json!(FULL_QUALITY);
    }
    payload(opcode::MEDIA_SINK_WANTS, data)
}

pub(crate) fn reply(reply: Reply) -> Outbound {
    match reply {
        Reply::KeyPackage(package) => binary(opcode::DAVE_KEY_PACKAGE, &[&package]),
        Reply::CommitWelcome { commit, welcome } => binary(
            opcode::DAVE_COMMIT_WELCOME,
            &[&commit, welcome.as_deref().unwrap_or_default()],
        ),
        Reply::TransitionReady(transition) => payload(
            opcode::DAVE_TRANSITION_READY,
            json!({ "transition_id": transition }),
        ),
        Reply::InvalidCommitWelcome(transition) => payload(
            opcode::DAVE_INVALID_COMMIT_WELCOME,
            json!({ "transition_id": transition }),
        ),
    }
}

fn offered(video: bool) -> Vec<Value> {
    let mut codecs = vec![json!({
        "name": "opus",
        "type": "audio",
        "priority": 1000,
        "payload_type": OPUS_PAYLOAD_TYPE,
    })];
    if video {
        codecs.extend(VIDEO_CODECS.iter().zip(1u32..).map(|(codec, rank)| {
            json!({
                "name": codec.name(),
                "type": "video",
                "priority": rank * 1000,
                "payload_type": Payload::Video(*codec).payload_type(),
                "rtx_payload_type": Payload::Repair(*codec).payload_type(),
                "encode": *codec == SENT_CODEC,
                "decode": true,
            })
        }));
    }
    codecs
}

fn payload(op: u8, data: Value) -> Outbound {
    Outbound::Text(json!({ "op": op, "d": data }).to_string())
}

fn binary(op: u8, parts: &[&[u8]]) -> Outbound {
    let mut frame = Vec::with_capacity(1 + parts.iter().map(|part| part.len()).sum::<usize>());
    frame.push(op);
    for part in parts {
        frame.extend_from_slice(part);
    }
    Outbound::Binary(frame)
}
