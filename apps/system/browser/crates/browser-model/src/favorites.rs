use std::collections::HashMap;

use rikka_ui::Observers;

use crate::ids::{FavoriteId, ProfileId};
use crate::saved::Saved;

#[derive(Clone, Debug, PartialEq)]
pub struct Favorite {
    pub id: FavoriteId,
    pub saved: Saved,
}

#[derive(Debug, Default)]
pub struct Favorites {
    by_profile: HashMap<ProfileId, Vec<Favorite>>,
    observers: Observers,
}

impl Favorites {
    pub fn for_profile(&self, profile: ProfileId) -> &[Favorite] {
        self.observers.read();
        self.by_profile.get(&profile).map_or(&[], Vec::as_slice)
    }

    pub fn order(&self, profile: ProfileId) -> Vec<FavoriteId> {
        self.by_profile
            .get(&profile)
            .map(|list| list.iter().map(|favorite| favorite.id).collect())
            .unwrap_or_default()
    }

    pub fn find(&self, id: FavoriteId) -> Option<(ProfileId, &Favorite)> {
        self.observers.read();
        self.by_profile.iter().find_map(|(profile, list)| {
            list.iter()
                .find(|favorite| favorite.id == id)
                .map(|favorite| (*profile, favorite))
        })
    }

    pub fn insert(&mut self, profile: ProfileId, at: usize, favorite: Favorite) {
        let list = self.by_profile.entry(profile).or_default();
        list.insert(at.min(list.len()), favorite);
        self.observers.changed();
    }

    pub fn remove(&mut self, id: FavoriteId) -> Option<(ProfileId, usize, Favorite)> {
        let (profile, at) = self.by_profile.iter().find_map(|(profile, list)| {
            list.iter()
                .position(|favorite| favorite.id == id)
                .map(|at| (*profile, at))
        })?;
        let favorite = self.by_profile.get_mut(&profile)?.remove(at);
        self.observers.changed();
        Some((profile, at, favorite))
    }

    pub fn place(&mut self, id: FavoriteId, at: usize) -> Option<ProfileId> {
        let (profile, from, favorite) = self.remove(id)?;
        let at = if at > from { at - 1 } else { at };
        self.insert(profile, at, favorite);
        Some(profile)
    }

    pub fn arrange(&mut self, profile: ProfileId, order: &[FavoriteId]) {
        let Some(list) = self.by_profile.get_mut(&profile) else {
            return;
        };
        let before: Vec<FavoriteId> = list.iter().map(|favorite| favorite.id).collect();
        list.sort_by_key(|favorite| {
            order
                .iter()
                .position(|id| *id == favorite.id)
                .unwrap_or(usize::MAX)
        });
        if list.iter().map(|favorite| favorite.id).ne(before) {
            self.observers.changed();
        }
    }

    pub fn retitle(&mut self, id: FavoriteId, url: &str, title: &str) {
        let entry = self
            .by_profile
            .values_mut()
            .flat_map(|list| list.iter_mut())
            .find(|favorite| favorite.id == id);
        let Some(favorite) = entry else {
            return;
        };
        if favorite.saved.url != url {
            favorite.saved.url = url.to_owned();
            favorite.saved.favicon = None;
        }
        favorite.saved.title = title.to_owned();
        self.observers.changed();
    }

    pub fn remember(&mut self, id: FavoriteId, saved: &Saved) {
        let entry = self
            .by_profile
            .values_mut()
            .flat_map(|list| list.iter_mut())
            .find(|favorite| favorite.id == id);
        if let Some(favorite) = entry
            && favorite.saved.favicon.is_none()
            && saved.favicon.is_some()
        {
            favorite.saved.favicon = saved.favicon.clone();
            self.observers.changed();
        }
    }
}
