use rikka_types::Choice;

use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.emoji";

pub const TYPE: &str = "type";
pub const SKIN_TONE: &str = "skin-tone";

pub const TYPE_DEFAULT: bool = true;

#[must_use]
pub fn schema<Tone: Choice>() -> Schema {
    Schema::new(DOMAIN)
        .summary("How the desktop's emoji picker puts an emoji where it is wanted")
        .key(Key::bool(TYPE, TYPE_DEFAULT).summary(
            "Whether choosing an emoji types it into the window that had the keyboard; \
             when it is off the emoji is copied to the clipboard instead",
        ))
        .key(
            Key::choice::<Tone>(SKIN_TONE)
                .summary("The skin tone the emoji that have one are shown and inserted with"),
        )
}
