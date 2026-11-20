use std::collections::HashMap;
use std::fmt;

use rikka_proton_cache::{Cache, Table};
use rikka_sync::Mutex;

use crate::account::Account;

const EVENT_CURSORS: Table<String> = Table::new("account.event-cursors");

pub struct Cursors {
    cache: Option<Cache>,
    loaded: Mutex<HashMap<String, String>>,
}

impl Cursors {
    #[must_use]
    pub fn for_account(account: &Account) -> Cursors {
        Cursors {
            cache: account.cache().cloned(),
            loaded: Mutex::default(),
        }
    }

    pub(crate) fn get(&self, cursor: &str) -> Option<String> {
        let mut loaded = self.loaded.lock();
        if let Some(id) = loaded.get(cursor) {
            return Some(id.clone());
        }
        let cached = self.cache.as_ref()?.get(&EVENT_CURSORS, cursor)?;
        loaded.insert(cursor.to_owned(), cached.clone());
        Some(cached)
    }

    pub(crate) fn set(&self, cursor: &str, id: &str) {
        if id.is_empty() {
            return;
        }
        self.loaded.lock().insert(cursor.to_owned(), id.to_owned());
        if let Some(cache) = &self.cache {
            cache.put(&EVENT_CURSORS, cursor, id.to_owned());
        }
    }
}

impl fmt::Debug for Cursors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Cursors")
            .field("cached", &self.cache.is_some())
            .finish_non_exhaustive()
    }
}
