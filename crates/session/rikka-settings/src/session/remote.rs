use std::ops::RangeInclusive;

use crate::key::Key;
use crate::schema::Schema;

pub const DOMAIN: &str = "rikka.remote";

pub const ENABLED: &str = "enabled";
pub const PORT: &str = "port";
pub const NETWORK: &str = "network";
pub const TAILSCALE: &str = "tailscale";
pub const MAX_BITRATE: &str = "max-bitrate";
pub const SOUND: &str = "sound";
pub const DISPLAY: &str = "display";

pub const ENABLED_DEFAULT: bool = false;
pub const PORT_DEFAULT: i64 = 47900;
pub const NETWORK_DEFAULT: bool = false;
pub const TAILSCALE_DEFAULT: bool = true;
pub const MAX_BITRATE_DEFAULT: i64 = 60;
pub const SOUND_DEFAULT: bool = true;

pub const FIRST_DISPLAY: &str = "";

pub const PORTS: RangeInclusive<i64> = 1024..=65535;
pub const MAX_BITRATES: RangeInclusive<i64> = 5..=300;

#[must_use]
pub fn schema() -> Schema {
    Schema::new(DOMAIN)
        .summary("Remote desktop: this session in the Rikka Remote viewer, over QUIC")
        .key(Key::bool(ENABLED, ENABLED_DEFAULT).summary(
            "Whether this session can be reached by a remote viewer; signing in takes the account password",
        ))
        .key(
            Key::int(PORT, PORT_DEFAULT)
                .summary("The UDP port viewers connect to")
                .range(PORTS),
        )
        .key(Key::bool(NETWORK, NETWORK_DEFAULT).summary(
            "Whether viewers anywhere on the network may connect; this computer and the tailnet are governed separately",
        ))
        .key(Key::bool(TAILSCALE, TAILSCALE_DEFAULT).summary(
            "Whether viewers on the tailnet may connect",
        ))
        .key(
            Key::int(MAX_BITRATE, MAX_BITRATE_DEFAULT)
                .summary("Megabits a second the picture may take at most; less is used when the link is slower")
                .range(MAX_BITRATES),
        )
        .key(Key::bool(SOUND, SOUND_DEFAULT).summary("Whether the desktop's sound is sent too"))
        .key(
            Key::string(DISPLAY, FIRST_DISPLAY)
                .summary("The display that is shown, by connector name; empty for the first one"),
        )
}
