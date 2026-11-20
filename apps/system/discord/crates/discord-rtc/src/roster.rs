use std::collections::{BTreeSet, HashMap};

use discord_core::UserId;

#[derive(Debug, Default)]
pub(crate) struct Roster {
    members: BTreeSet<UserId>,
    sources: HashMap<u32, UserId>,
}

impl Roster {
    pub(crate) fn join(&mut self, user: UserId) {
        self.members.insert(user);
    }

    pub(crate) fn source(&mut self, ssrc: u32, user: UserId) -> bool {
        if ssrc == 0 {
            self.join(user);
            return false;
        }
        self.members.insert(user);
        self.sources.insert(ssrc, user) != Some(user)
    }

    pub(crate) fn leave(&mut self, user: UserId) -> Vec<u32> {
        self.members.remove(&user);
        let mut departed: Vec<u32> = self
            .sources
            .iter()
            .filter(|(_, owner)| **owner == user)
            .map(|(&ssrc, _)| ssrc)
            .collect();
        departed.sort_unstable();
        for ssrc in &departed {
            self.sources.remove(ssrc);
        }
        departed
    }

    pub(crate) fn user(&self, ssrc: u32) -> Option<UserId> {
        self.sources.get(&ssrc).copied()
    }

    pub(crate) fn sources(&self) -> Vec<(u32, UserId)> {
        let mut sources: Vec<(u32, UserId)> = self
            .sources
            .iter()
            .map(|(&ssrc, &user)| (ssrc, user))
            .collect();
        sources.sort_unstable();
        sources
    }

    pub(crate) fn members(&self, me: u64) -> Vec<u64> {
        self.members
            .iter()
            .map(|user| user.get())
            .chain([me])
            .collect()
    }
}
