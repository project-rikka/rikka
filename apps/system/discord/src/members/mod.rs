mod roster;

use discord_core::{ChannelId, ChannelKind};
use rikka_app::prelude::*;

use crate::chat::Chat;
use crate::profile;

pub(crate) const WIDTH: f32 = 240.0;
pub(crate) fn view(chat: &Chat, channel: &ChannelId) -> Element {
    let Some(linked) = chat.current() else {
        return spacer().any();
    };
    let Some(channel_info) = linked.channel(channel) else {
        return spacer().any();
    };
    let body = match (channel_info.kind, channel_info.recipients.first()) {
        (ChannelKind::Direct, Some(peer)) => {
            profile::panel(chat, linked.key(), peer, WIDTH).unwrap_or_else(|| spacer().any())
        }
        (ChannelKind::Group, _) => roster::recipients(linked, channel_info),
        _ => roster::view(linked, channel_info),
    };
    container(body)
        .background(palette().surface)
        .width(WIDTH)
        .fill_height()
        .any()
}
