use serde::Deserialize;

use crate::flex;
use crate::guild::Member;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct VoiceState {
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub user_id: String,
    pub member: Option<Member>,
    pub session_id: Option<String>,
    #[serde(deserialize_with = "flex::flag")]
    pub self_mute: bool,
    #[serde(deserialize_with = "flex::flag")]
    pub self_deaf: bool,
    #[serde(deserialize_with = "flex::flag")]
    pub mute: bool,
    #[serde(deserialize_with = "flex::flag")]
    pub deaf: bool,
    #[serde(deserialize_with = "flex::flag")]
    pub suppress: bool,
    #[serde(deserialize_with = "flex::flag")]
    pub self_stream: bool,
    #[serde(deserialize_with = "flex::flag")]
    pub self_video: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct VoiceServer {
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub token: String,
    pub endpoint: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Call {
    pub channel_id: String,
    #[serde(deserialize_with = "flex::list")]
    pub ringing: Vec<String>,
    #[serde(deserialize_with = "flex::list")]
    pub voice_states: Vec<VoiceState>,
    #[serde(deserialize_with = "flex::flag")]
    pub unavailable: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Stream {
    pub stream_key: String,
    pub rtc_server_id: Option<String>,
    pub rtc_channel_id: Option<String>,
    #[serde(deserialize_with = "flex::list")]
    pub viewer_ids: Vec<String>,
    #[serde(deserialize_with = "flex::flag")]
    pub paused: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct StreamServer {
    pub stream_key: String,
    pub token: String,
    pub endpoint: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct StreamDelete {
    pub stream_key: String,
    pub reason: Option<String>,
    #[serde(deserialize_with = "flex::flag")]
    pub unavailable: bool,
}
