use std::ops::RangeInclusive;

use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.lock";

pub const IDLE: &str = "idle";
pub const BEFORE_SLEEP: &str = "before-sleep";
pub const AT_LOGIN: &str = "at-login";

pub const IDLE_DEFAULT: i64 = 10;
pub const BEFORE_SLEEP_DEFAULT: bool = true;
pub const AT_LOGIN_DEFAULT: bool = true;

pub const IDLES: RangeInclusive<i64> = 0..=240;

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("When the screen locks by itself")
        .key(
            Key::int(IDLE, IDLE_DEFAULT)
                .summary("Minutes without input before the screen locks; 0 never locks it")
                .range(IDLES),
        )
        .key(
            Key::bool(BEFORE_SLEEP, BEFORE_SLEEP_DEFAULT)
                .summary("Whether the screen locks before the computer goes to sleep"),
        )
        .key(
            Key::bool(AT_LOGIN, AT_LOGIN_DEFAULT)
                .summary("Whether a session started on the console begins at the lock screen"),
        )
}
