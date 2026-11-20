use discord_core::{ChannelId, SpaceId, UserId, VoiceState};

use discord_wire as wire;
use discord_wire::snowflake;

pub fn voice_state(wire: &wire::VoiceState) -> Option<VoiceState> {
    let channel = wire.channel_id.as_deref()?;
    Some(VoiceState {
        user: snowflake::id::<UserId>(wire.user_id.as_str()),
        space: wire.guild_id.as_deref().map(snowflake::id::<SpaceId>),
        channel: snowflake::id::<ChannelId>(channel),
        self_muted: wire.self_mute,
        self_deafened: wire.self_deaf,
        muted: wire.mute,
        deafened: wire.deaf,
        suppressed: wire.suppress,
        streaming: wire.self_stream,
        video: wire.self_video,
    })
}
