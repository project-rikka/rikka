use std::collections::{HashMap, HashSet};

use discord_core::{ChannelId, SpaceId};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Scope {
    Home,
    Space(SpaceId),
}

impl Scope {
    pub(crate) fn space(&self) -> Option<&SpaceId> {
        match self {
            Scope::Home => None,
            Scope::Space(space) => Some(space),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Location {
    pub(crate) scope: Scope,
    pub(crate) channel: Option<ChannelId>,
}

impl Location {
    pub(crate) fn home() -> Location {
        Location {
            scope: Scope::Home,
            channel: None,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Navigation {
    location: Location,
    last: HashMap<Scope, ChannelId>,
    unfolded: HashSet<String>,
}

impl Default for Navigation {
    fn default() -> Navigation {
        Navigation {
            location: Location::home(),
            last: HashMap::new(),
            unfolded: HashSet::new(),
        }
    }
}

impl Navigation {
    pub(crate) fn location(&self) -> &Location {
        &self.location
    }

    pub(crate) fn last_in(&self, scope: &Scope) -> Option<&ChannelId> {
        self.last.get(scope)
    }

    pub(crate) fn go(&mut self, location: Location) -> bool {
        if let Some(channel) = &location.channel {
            self.last.insert(location.scope.clone(), *channel);
        }
        if self.location == location {
            return false;
        }
        self.location = location;
        true
    }

    pub(crate) fn forget_channel(&mut self, channel: &ChannelId) -> bool {
        self.last.retain(|_, last| last != channel);
        if self.location.channel.as_ref() != Some(channel) {
            return false;
        }
        self.location.channel = None;
        true
    }

    pub(crate) fn forget_space(&mut self, space: &SpaceId) -> bool {
        let scope = Scope::Space(*space);
        self.last.remove(&scope);
        if self.location.scope != scope {
            return false;
        }
        self.location = Location::home();
        true
    }

    pub(crate) fn is_unfolded(&self, folder: &str) -> bool {
        self.unfolded.contains(folder)
    }

    pub(crate) fn toggle_folder(&mut self, folder: &str) {
        if !self.unfolded.remove(folder) {
            self.unfolded.insert(folder.to_owned());
        }
    }
}
