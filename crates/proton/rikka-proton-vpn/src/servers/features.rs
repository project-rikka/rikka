use std::fmt;
use std::ops::{BitOr, BitOrAssign};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ServerFeatures(u32);

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Tier(u8);

const FEATURE_NAMES: [(ServerFeatures, &str); 7] = [
    (ServerFeatures::SECURE_CORE, "secure core"),
    (ServerFeatures::TOR, "tor"),
    (ServerFeatures::P2P, "p2p"),
    (ServerFeatures::STREAMING, "streaming"),
    (ServerFeatures::IPV6, "ipv6"),
    (ServerFeatures::RESTRICTED, "restricted"),
    (ServerFeatures::PARTNER, "partner"),
];

impl ServerFeatures {
    pub const SECURE_CORE: ServerFeatures = ServerFeatures(1);
    pub const TOR: ServerFeatures = ServerFeatures(2);
    pub const P2P: ServerFeatures = ServerFeatures(4);
    pub const STREAMING: ServerFeatures = ServerFeatures(8);
    pub const IPV6: ServerFeatures = ServerFeatures(16);
    pub const RESTRICTED: ServerFeatures = ServerFeatures(32);
    pub const PARTNER: ServerFeatures = ServerFeatures(64);

    #[must_use]
    pub fn from_bits(bits: u32) -> ServerFeatures {
        ServerFeatures(bits)
    }

    #[must_use]
    pub fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn contains(self, features: ServerFeatures) -> bool {
        self.0 & features.0 == features.0
    }

    #[must_use]
    pub fn is_secure_core(self) -> bool {
        self.contains(ServerFeatures::SECURE_CORE)
    }

    #[must_use]
    pub fn is_tor(self) -> bool {
        self.contains(ServerFeatures::TOR)
    }

    #[must_use]
    pub fn allows_p2p(self) -> bool {
        self.contains(ServerFeatures::P2P)
    }

    #[must_use]
    pub fn allows_streaming(self) -> bool {
        self.contains(ServerFeatures::STREAMING)
    }

    #[must_use]
    pub fn supports_ipv6(self) -> bool {
        self.contains(ServerFeatures::IPV6)
    }

    #[must_use]
    pub fn is_restricted(self) -> bool {
        self.contains(ServerFeatures::RESTRICTED)
    }

    #[must_use]
    pub fn is_partner(self) -> bool {
        self.contains(ServerFeatures::PARTNER)
    }
}

impl Tier {
    pub const FREE: Tier = Tier(0);
    pub const BASIC: Tier = Tier(1);
    pub const PLUS: Tier = Tier(2);
    pub const INTERNAL: Tier = Tier(3);

    #[must_use]
    pub fn new(level: u8) -> Tier {
        Tier(level)
    }

    #[must_use]
    pub fn level(self) -> u8 {
        self.0
    }

    #[must_use]
    pub fn includes(self, server: Tier) -> bool {
        self.0 >= server.0
    }
}

impl BitOr for ServerFeatures {
    type Output = ServerFeatures;

    fn bitor(self, features: ServerFeatures) -> ServerFeatures {
        ServerFeatures(self.0 | features.0)
    }
}

impl BitOrAssign for ServerFeatures {
    fn bitor_assign(&mut self, features: ServerFeatures) {
        self.0 |= features.0;
    }
}

impl fmt::Debug for ServerFeatures {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = FEATURE_NAMES
            .iter()
            .filter(|(feature, _)| self.contains(*feature))
            .map(|(_, name)| *name)
            .collect();

        write!(f, "ServerFeatures({})", names.join(" | "))
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Tier::FREE => f.write_str("free"),
            Tier::BASIC => f.write_str("basic"),
            Tier::PLUS => f.write_str("plus"),
            Tier::INTERNAL => f.write_str("internal"),
            Tier(level) => write!(f, "tier {level}"),
        }
    }
}
