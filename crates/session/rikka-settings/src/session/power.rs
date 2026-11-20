use std::ops::RangeInclusive;

use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.power";

pub const SCREEN_OFF_PLUGGED: &str = "screen-off-plugged";
pub const SCREEN_OFF_BATTERY: &str = "screen-off-battery";
pub const SUSPEND_PLUGGED: &str = "suspend-plugged";
pub const SUSPEND_BATTERY: &str = "suspend-battery";
pub const DIM: &str = "dim";
pub const SAVER_ON_LOW_BATTERY: &str = "saver-on-low-battery";
pub const BATTERY_PERCENTAGE: &str = "battery-percentage";

pub const SCREEN_OFF_PLUGGED_DEFAULT: i64 = 10;
pub const SCREEN_OFF_BATTERY_DEFAULT: i64 = 5;
pub const SUSPEND_PLUGGED_DEFAULT: i64 = 30;
pub const SUSPEND_BATTERY_DEFAULT: i64 = 15;
pub const DIM_DEFAULT: bool = true;
pub const SAVER_ON_LOW_BATTERY_DEFAULT: bool = true;
pub const BATTERY_PERCENTAGE_DEFAULT: bool = false;

pub const MINUTES: RangeInclusive<i64> = 0..=240;

#[must_use]
pub fn schema() -> Schema {
    let minutes = |name: &str, default: i64, summary: &str| {
        Key::int(name, default).summary(summary).range(MINUTES)
    };
    Schema::new(DOMAIN)
        .summary("When the screen turns off and the computer sleeps, and how it saves power")
        .key(minutes(
            SCREEN_OFF_PLUGGED,
            SCREEN_OFF_PLUGGED_DEFAULT,
            "Minutes without input before the screen turns off on external power; 0 never turns it off",
        ))
        .key(minutes(
            SCREEN_OFF_BATTERY,
            SCREEN_OFF_BATTERY_DEFAULT,
            "Minutes without input before the screen turns off on battery; 0 never turns it off",
        ))
        .key(minutes(
            SUSPEND_PLUGGED,
            SUSPEND_PLUGGED_DEFAULT,
            "Minutes without input before the computer sleeps on external power; 0 never sleeps",
        ))
        .key(minutes(
            SUSPEND_BATTERY,
            SUSPEND_BATTERY_DEFAULT,
            "Minutes without input before the computer sleeps on battery; 0 never sleeps",
        ))
        .key(
            Key::bool(DIM, DIM_DEFAULT)
                .summary("Whether the screen dims shortly before it turns off"),
        )
        .key(
            Key::bool(SAVER_ON_LOW_BATTERY, SAVER_ON_LOW_BATTERY_DEFAULT)
                .summary("Whether power saver turns on by itself while the battery is low"),
        )
        .key(
            Key::bool(BATTERY_PERCENTAGE, BATTERY_PERCENTAGE_DEFAULT)
                .summary("Whether the taskbar shows the battery charge as a percentage"),
        )
}
