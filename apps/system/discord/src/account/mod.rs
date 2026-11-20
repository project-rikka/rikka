mod marks;
mod navigation;
mod reading;

use discord_client::{Client, Link};
use discord_core::{Account, Area, ChannelId, Store, Update, Updates};
use rikka_ui::Revision;

use marks::Marks;
use navigation::Navigation;
pub(crate) use navigation::{Location, Scope};

pub(crate) struct LinkedAccount {
    key: String,
    account: Account,
    client: Client,
    store: Store,
    marks: Marks,
    navigation: Navigation,
    moved: Revision,
}

impl LinkedAccount {
    pub(crate) fn new(key: &str, link: Link) -> (LinkedAccount, Updates) {
        let linked = LinkedAccount {
            key: key.to_owned(),
            account: link.account,
            client: link.client,
            store: Store::default(),
            marks: Marks::default(),
            navigation: Navigation::default(),
            moved: Revision::new(),
        };
        (linked, link.updates)
    }

    pub(crate) fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn account(&self) -> &Account {
        &self.account
    }

    pub(crate) fn client(&self) -> &Client {
        &self.client
    }

    pub(crate) fn describe(&mut self, account: Account) {
        self.account = account;
    }

    pub(crate) fn apply(&mut self, update: Update) -> Vec<Area> {
        let removed = match &update {
            Update::ChannelGone(channel) => self.navigation.forget_channel(channel),
            Update::SpaceGone(space) => self.navigation.forget_space(space),
            _ => false,
        };
        if removed {
            self.moved.changed();
        }
        self.change(|store| store.apply(update))
    }

    pub(crate) fn change(&mut self, change: impl FnOnce(&mut Store) -> Vec<Area>) -> Vec<Area> {
        let areas = change(&mut self.store);
        self.marks.invalidate(&areas);
        for area in &areas {
            if let Area::Channel(channel) = area
                && self.store.channel(channel).is_none()
            {
                self.marks.forget(channel);
            }
        }
        areas
    }

    pub(crate) fn location(&self) -> &Location {
        self.moved.read();
        self.navigation.location()
    }

    pub(crate) fn last_in(&self, scope: &Scope) -> Option<&ChannelId> {
        self.navigation.last_in(scope)
    }

    pub(crate) fn go(&mut self, location: Location) -> bool {
        let moved = self.navigation.go(location);
        if moved {
            self.moved.changed();
        }
        moved
    }

    pub(crate) fn is_unfolded(&self, folder: &str) -> bool {
        self.moved.read();
        self.navigation.is_unfolded(folder)
    }

    pub(crate) fn toggle_folder(&mut self, folder: &str) {
        self.navigation.toggle_folder(folder);
        self.moved.changed();
    }
}

impl std::fmt::Debug for LinkedAccount {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LinkedAccount")
            .field("key", &self.key)
            .field("account", &self.account)
            .finish_non_exhaustive()
    }
}
