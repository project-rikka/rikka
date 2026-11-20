use std::ops::RangeInclusive;

use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.notifications";

pub const DO_NOT_DISTURB: &str = "do-not-disturb";
pub const TIMEOUT: &str = "timeout";
pub const VISIBLE: &str = "visible";

pub const DO_NOT_DISTURB_DEFAULT: bool = false;
pub const TIMEOUT_DEFAULT: i64 = 5;
pub const VISIBLE_DEFAULT: i64 = 3;

pub const TIMEOUTS: RangeInclusive<i64> = 1..=300;
pub const VISIBLES: RangeInclusive<i64> = 1..=6;

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("What the desktop does with what applications tell it")
        .key(Key::bool(DO_NOT_DISTURB, DO_NOT_DISTURB_DEFAULT).summary(
            "Whether notifications are kept off the screen; urgent ones come through anyway",
        ))
        .key(
            Key::int(TIMEOUT, TIMEOUT_DEFAULT)
                .summary("Seconds a notification stands for when the application named no time")
                .range(TIMEOUTS),
        )
        .key(
            Key::int(VISIBLE, VISIBLE_DEFAULT)
                .summary("How many notifications are on screen at once; the rest wait their turn")
                .range(VISIBLES),
        )
}
