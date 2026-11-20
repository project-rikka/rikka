use std::ops::RangeInclusive;

use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.clipboard";

pub const HISTORY: &str = "history";
pub const PASTE: &str = "paste";

pub const HISTORY_DEFAULT: i64 = 100;
pub const PASTE_DEFAULT: bool = true;

pub const HISTORIES: RangeInclusive<i64> = 1..=1000;

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("Clipboard history")
        .key(
            Key::int(HISTORY, HISTORY_DEFAULT)
                .summary("How many clippings are kept; pinned ones are kept beyond it")
                .range(HISTORIES),
        )
        .key(
            Key::bool(PASTE, PASTE_DEFAULT).summary(
                "Whether choosing a clipping pastes it into the window that had the keyboard",
            ),
        )
}
