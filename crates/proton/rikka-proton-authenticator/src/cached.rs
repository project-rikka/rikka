use rikka_proton_cache::{Cache, SINGLETON_ID, Table, Wired};

use crate::api::RemoteEntry;
use crate::api::SyncKey;
use crate::api::entry::EntryPayload;
use crate::api::key::KeyPayload;
use crate::client::{AuthenticatorClient, decrypt_entries, into_values};
use crate::error::Result;

const KEYS: Table<Vec<Wired<KeyPayload>>> = Table::new("authenticator.keys");
const ENTRIES: Table<Vec<Wired<EntryPayload>>> = Table::new("authenticator.entries");

impl AuthenticatorClient {
    #[must_use]
    pub fn with_cache(mut self, cache: Cache) -> AuthenticatorClient {
        self.account = self.account.with_cache(cache);
        self
    }

    #[must_use]
    pub fn cache(&self) -> Option<&Cache> {
        self.account.cache()
    }

    pub async fn cached_keys(&self) -> Result<Option<Vec<SyncKey>>> {
        let Some(cached) = self
            .cache()
            .and_then(|cache| cache.get(&KEYS, SINGLETON_ID))
        else {
            return Ok(None);
        };
        Ok(Some(self.decrypt_keys(into_values(cached)).await?))
    }

    pub async fn cached_entries(&self, keys: &[SyncKey]) -> Option<Vec<RemoteEntry>> {
        let cached = self.cache()?.get(&ENTRIES, SINGLETON_ID)?;
        let (entries, _) = decrypt_entries(into_values(cached), keys).await;
        Some(entries)
    }

    pub(crate) fn cache_keys(&self, keys: &[Wired<KeyPayload>]) {
        if let Some(cache) = self.cache() {
            cache.put(&KEYS, SINGLETON_ID, keys.to_vec());
        }
    }

    pub(crate) fn add_cached_key(&self, key: &Wired<KeyPayload>) {
        let Some(cache) = self.cache() else {
            return;
        };
        let mut cached = cache.get(&KEYS, SINGLETON_ID).unwrap_or_default();
        cached.retain(|cached| cached.id != key.id);
        cached.push(key.clone());
        cache.put(&KEYS, SINGLETON_ID, cached);
    }

    pub(crate) fn cache_entries(&self, entries: &[Wired<EntryPayload>]) {
        if let Some(cache) = self.cache() {
            cache.put(&ENTRIES, SINGLETON_ID, entries.to_vec());
        }
    }

    pub(crate) fn upsert_cached_entries(&self, entries: &[Wired<EntryPayload>]) {
        self.update_cached_entries(|cached| {
            for entry in entries {
                match cached.iter_mut().find(|cached| cached.id == entry.id) {
                    Some(cached) => *cached = entry.clone(),
                    None => cached.push(entry.clone()),
                }
            }
        });
    }

    pub(crate) fn remove_cached_entries(&self, ids: &[String]) {
        self.update_cached_entries(|cached| cached.retain(|cached| !ids.contains(&cached.id)));
    }

    pub(crate) fn reorder_cached_entries(&self, starting_position: u32, ids: &[String]) {
        self.update_cached_entries(|cached| {
            let moved: Vec<Wired<EntryPayload>> = ids
                .iter()
                .filter_map(|id| cached.iter().find(|cached| &cached.id == id).cloned())
                .collect();
            cached.retain(|cached| !ids.contains(&cached.id));
            let start = (starting_position as usize).min(cached.len());
            cached.splice(start..start, moved);
        });
    }

    fn update_cached_entries(&self, update: impl FnOnce(&mut Vec<Wired<EntryPayload>>)) {
        let Some(cache) = self.cache() else {
            return;
        };
        let Some(mut cached) = cache.get(&ENTRIES, SINGLETON_ID) else {
            return;
        };
        update(&mut cached);
        cache.put(&ENTRIES, SINGLETON_ID, cached);
    }
}
