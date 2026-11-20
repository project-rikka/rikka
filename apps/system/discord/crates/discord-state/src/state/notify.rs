use discord_wire::notification_level;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Arrival {
    pub(crate) from_me: bool,
    pub(crate) from_blocked: bool,
    pub(crate) silent: bool,
    pub(crate) private: bool,
    pub(crate) busy: bool,
    pub(crate) muted: bool,
    pub(crate) level: u8,
    pub(crate) direct: bool,
    pub(crate) everyone: bool,
    pub(crate) role: bool,
    pub(crate) suppress_everyone: bool,
    pub(crate) suppress_roles: bool,
}

impl Arrival {
    pub(crate) fn mentions_me(&self) -> bool {
        self.direct
            || (self.everyone && !self.suppress_everyone)
            || (self.role && !self.suppress_roles)
    }

    pub(crate) fn counts_as_mention(&self) -> bool {
        !self.from_me && (self.private || self.mentions_me())
    }

    pub(crate) fn notifies(&self) -> bool {
        if self.from_me || self.from_blocked || self.silent || self.busy || self.muted {
            return false;
        }
        if self.private {
            return true;
        }
        match self.level {
            notification_level::ALL => true,
            notification_level::MENTIONS => self.mentions_me(),
            _ => false,
        }
    }
}

pub(crate) fn resolve(levels: &[u8], fallback: u8) -> u8 {
    levels
        .iter()
        .copied()
        .find(|&chosen| chosen != notification_level::INHERIT)
        .unwrap_or(fallback)
}
