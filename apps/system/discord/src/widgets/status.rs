use discord_core::Status;
use rikka_app::prelude::*;

pub(crate) const CHOSEN: [Status; 4] = [
    Status::Online,
    Status::Idle,
    Status::Busy,
    Status::Invisible,
];

pub(crate) fn status_color(status: Status) -> Color {
    let palette = palette();
    match status {
        Status::Online => palette.success,
        Status::Idle => palette.warning,
        Status::Busy => palette.danger,
        Status::Invisible | Status::Offline => palette.text_muted,
    }
}

pub(crate) fn status_label(status: Status) -> Label {
    match status {
        Status::Online => tr!("discord.status.online"),
        Status::Idle => tr!("discord.status.idle"),
        Status::Busy => tr!("discord.status.busy"),
        Status::Invisible => tr!("discord.status.invisible"),
        Status::Offline => tr!("discord.status.offline"),
    }
}
