use nixpkgs_search::{Error, Nixpkgs, Result};
use rikka_nix::Channel;

#[derive(Debug, Default)]
pub(super) enum Backend {
    #[default]
    Closed,
    Opening,
    Open {
        channel: Channel,
        nixpkgs: Nixpkgs,
    },
    Unavailable(Error),
}

impl Backend {
    pub(super) fn opened(opened: Result<(Channel, Nixpkgs)>) -> Self {
        match opened {
            Ok((channel, nixpkgs)) => Self::Open { channel, nixpkgs },
            Err(error) => Self::Unavailable(error),
        }
    }

    pub(super) fn is_opening(&self) -> bool {
        matches!(self, Self::Opening)
    }

    pub(super) fn nixpkgs(&self) -> Option<&Nixpkgs> {
        match self {
            Self::Open { nixpkgs, .. } => Some(nixpkgs),
            _ => None,
        }
    }

    pub(super) fn channel(&self) -> Option<&Channel> {
        match self {
            Self::Open { channel, .. } => Some(channel),
            _ => None,
        }
    }

    pub(super) fn unavailable(&self) -> Option<&Error> {
        match self {
            Self::Unavailable(error) => Some(error),
            _ => None,
        }
    }
}
