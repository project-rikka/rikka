mod cdn;
mod channel;
mod guild;
mod message;
mod profile;
mod user;
mod voice;

pub use cdn::avatar;
pub use channel::{Looks, channel};
pub use guild::{custom_emoji, reaction_emoji, role, space, sticker};
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
