mod cdn;
mod channel;
mod guild;
mod manage;
mod message;
mod profile;
mod user;
mod voice;

pub use cdn::avatar;
pub use channel::{Looks, channel, kind, kind_code};
pub use guild::{SpaceLooks, custom_emoji, reaction_emoji, role, space, sticker};
pub use manage::{
    audit_log, automod_rule, ban, channel_settings, incidents, integration, invite, member_hit,
    mfa_challenge, notify_code, notify_level, onboarding, overwrite, space_emoji, space_settings,
    space_sound, space_sticker, template, vanity, webhook, welcome_screen, widget,
};
pub use message::{Reader, embeds, message};
pub use profile::profile;
pub use user::{friend, member, presence, user};
pub use voice::voice_state;

#[must_use]
pub fn instant(text: Option<&str>) -> Option<i64> {
    let parsed: jiff::Timestamp = text?.parse().ok()?;
    Some(parsed.as_millisecond())
}

pub(crate) fn filled(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text| !text.is_empty())
}
