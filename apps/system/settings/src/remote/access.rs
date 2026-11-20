use rikka_nix::{Document, Module, Set};

use crate::pending::Pending;

const MODULE: Module = Module::Desktop;

const PAM: &str = "security.pam.services.rikka-remote";
const TAILNET_UDP: &str = "networking.firewall.interfaces.tailscale0.allowedUDPPorts";
const UDP: &str = "networking.firewall.allowedUDPPorts";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Opening {
    pub(crate) port: i64,
    pub(crate) network: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Access {
    Unknown,
    Closed,
    Opening,
    Open,
    Closing,
}

impl Opening {
    pub(crate) fn access(self, pending: &Pending) -> Access {
        let (Some(edited), Some(saved)) = (pending.edited(MODULE), pending.saved(MODULE)) else {
            return Access::Unknown;
        };
        match (self.is_open(saved), self.is_open(edited)) {
            (true, true) => Access::Open,
            (false, true) => Access::Opening,
            (true, false) => Access::Closing,
            (false, false) => Access::Closed,
        }
    }

    pub(crate) fn allow(self, pending: &mut Pending, on: bool) {
        pending.change(MODULE, |document| match on {
            true => self.open(document),
            false => self.close(document),
        });
    }

    pub(crate) fn follow(self, pending: &mut Pending, moved: Opening) {
        if self == moved
            || !pending
                .edited(MODULE)
                .is_some_and(|edited| self.is_open(edited))
        {
            return;
        }
        pending.change(MODULE, |document| {
            self.close(document)?;
            moved.open(document)
        });
    }

    fn is_open(self, document: &Document) -> bool {
        let holds = |path: &str| {
            document
                .get::<Vec<i64>>(path)
                .ok()
                .flatten()
                .is_some_and(|ports| ports.contains(&self.port))
        };
        holds(TAILNET_UDP) && (!self.network || holds(UDP))
    }

    fn open(self, document: &mut Document) -> rikka_nix::Result<()> {
        if document.get::<Set>(PAM)?.is_none() {
            document.set(PAM, Set::new())?;
        }
        document.include(TAILNET_UDP, self.port)?;
        match self.network {
            true => document.include(UDP, self.port)?,
            false => document.exclude(UDP, self.port)?,
        }
        Ok(())
    }

    fn close(self, document: &mut Document) -> rikka_nix::Result<()> {
        for path in [TAILNET_UDP, UDP] {
            document.exclude(path, self.port)?;
        }
        Ok(())
    }
}
