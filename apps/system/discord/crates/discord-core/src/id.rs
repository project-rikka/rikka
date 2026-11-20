use std::num::ParseIntError;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const DISCORD_EPOCH_MS: i64 = 1_420_070_400_000;
const TIMESTAMP_SHIFT: u32 = 22;

macro_rules! snowflakes {
    ($($name:ident),* $(,)?) => {$(
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn new(raw: u64) -> $name {
                $name(raw)
            }

            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }

            #[must_use]
            pub fn created_at(self) -> i64 {
                DISCORD_EPOCH_MS + (self.0 >> TIMESTAMP_SHIFT) as i64
            }
        }

        impl From<u64> for $name {
            fn from(raw: u64) -> $name {
                $name(raw)
            }
        }

        impl FromStr for $name {
            type Err = ParseIntError;

            fn from_str(text: &str) -> Result<$name, ParseIntError> {
                text.parse().map($name)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}", self.0)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, concat!(stringify!($name), "({})"), self.0)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.collect_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<$name, D::Error> {
                deserializer.deserialize_any(Raw).map($name)
            }
        }
    )*};
}

snowflakes!(
    SpaceId, ChannelId, MessageId, UserId, RoleId, EmojiId, StickerId, TagId
);

impl MessageId {
    #[must_use]
    pub fn minted_at(millis: i64) -> MessageId {
        let since = u64::try_from(millis.saturating_sub(DISCORD_EPOCH_MS)).unwrap_or(0);
        MessageId(since << TIMESTAMP_SHIFT)
    }
}

struct Raw;

impl Visitor<'_> for Raw {
    type Value = u64;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a Discord snowflake")
    }

    fn visit_u64<E: de::Error>(self, raw: u64) -> Result<u64, E> {
        Ok(raw)
    }

    fn visit_i64<E: de::Error>(self, raw: i64) -> Result<u64, E> {
        u64::try_from(raw).map_err(E::custom)
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<u64, E> {
        text.parse().map_err(E::custom)
    }
}
