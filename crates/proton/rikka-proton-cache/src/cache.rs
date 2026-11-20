use rikka_sync::Mutex;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};

use rikka_proton_pgp::SymmetricKey;
use rusqlite::Connection;

use crate::batch::Batch;
use crate::blob::BlobArea;
use crate::error::{Error, Result};
use crate::keys::Keys;
use crate::overlay::{CacheKey, Change, Lookup, Overlay, Value};
use crate::slot::Slot;
use crate::store::{self, Location};
use crate::table::{Record, Table};
use crate::writer::{self, Message, Op, State};

const MEMORY: usize = 4096;

#[derive(Clone)]
pub struct Cache {
    shared: Arc<Shared>,
    era: u64,
}

struct Shared {
    state: Arc<State>,
    keys: Keys,
    location: Location,
    reader: Mutex<Connection>,
    messages: Sender<Message>,
    seq: AtomicU64,
    used: AtomicU64,
}

pub(crate) struct PendingWrite {
    pub(crate) change: Change,
    pub(crate) op: Op,
}

impl Cache {
    pub fn open(dir: impl Into<PathBuf>, key: SymmetricKey) -> Result<Cache> {
        Cache::open_at(Location::folder(&dir.into())?, &key)
    }

    pub fn in_memory(key: SymmetricKey) -> Result<Cache> {
        Cache::open_at(Location::memory(), &key)
    }

    fn open_at(location: Location, key: &SymmetricKey) -> Result<Cache> {
        let keys = Keys::derive(key)?;
        let writer = location.writer()?;
        let reader = location.reader()?;
        let used = store::last_use(&writer)?;
        let state = Arc::new(State {
            overlay: Mutex::new(Overlay::new(MEMORY)),
            era: AtomicU64::new(0),
        });
        let (messages, receiver) = mpsc::channel();
        writer::spawn(writer, Arc::clone(&state), receiver)?;

        Ok(Cache {
            shared: Arc::new(Shared {
                state,
                keys,
                location,
                reader: Mutex::new(reader),
                messages,
                seq: AtomicU64::new(0),
                used: AtomicU64::new(used),
            }),
            era: 0,
        })
    }

    #[must_use]
    pub fn current_era(&self) -> Cache {
        Cache {
            shared: Arc::clone(&self.shared),
            era: self.shared.state.era.load(Ordering::SeqCst),
        }
    }

    #[must_use]
    pub fn is_current(&self) -> bool {
        self.era == self.shared.state.era.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.shared.location.path()
    }

    #[must_use]
    pub fn get<T: Record>(&self, table: &Table<T>, id: &str) -> Option<T> {
        let cache_key = CacheKey::new(table.name(), id);
        let generation = {
            let mut overlay = self.shared.state.overlay();
            match overlay.get(&cache_key) {
                Lookup::Hit(value) => match value.downcast_ref::<T>() {
                    Some(value) => {
                        let value = value.clone();
                        drop(overlay);
                        self.touch(table, self.shared.keys.record_key(table.name(), id));
                        return Some(value);
                    }
                    None => overlay.generation(),
                },
                Lookup::Removed => return None,
                Lookup::Miss => overlay.generation(),
            }
        };

        let key = self.shared.keys.record_key(table.name(), id);
        let sealed = self
            .read(|connection| store::record(connection, table.name(), &key))
            .flatten()?;
        let (found, value) = self.decrypt_record::<T>(table, &key, &sealed)?;
        if found != id {
            return None;
        }
        self.touch(table, key);
        self.shared
            .state
            .overlay()
            .fill(cache_key, Arc::new(value.clone()), generation);
        Some(value)
    }

    #[must_use]
    pub fn contains<T: Record>(&self, table: &Table<T>, id: &str) -> bool {
        self.get(table, id).is_some()
    }

    #[must_use]
    pub fn entries<T: Record>(&self, table: &Table<T>) -> Vec<(String, T)> {
        let pending = self.shared.state.overlay().pending(table.name());
        let mut merged: BTreeMap<String, T> = BTreeMap::new();

        if !pending.hidden {
            let rows = self
                .read(|connection| store::records(connection, table.name()))
                .unwrap_or_default();
            for (key, sealed) in rows {
                if let Some((id, value)) = self.decrypt_record::<T>(table, &key, &sealed) {
                    merged.insert(id, value);
                }
            }
        }

        for (id, value) in pending.records {
            match value.as_deref().and_then(|value| value.downcast_ref::<T>()) {
                Some(value) => {
                    merged.insert(id.to_string(), value.clone());
                }
                None => {
                    merged.remove(&*id);
                }
            }
        }

        merged.into_iter().collect()
    }

    #[must_use]
    pub fn values<T: Record>(&self, table: &Table<T>) -> Vec<T> {
        self.entries(table)
            .into_iter()
            .map(|(_, value)| value)
            .collect()
    }

    pub fn put<T: Record>(&self, table: &Table<T>, id: &str, value: T) {
        let mut batch = self.batch();
        batch.put(table, id, value);
        batch.commit();
    }

    pub fn remove<T: Record>(&self, table: &Table<T>, id: &str) {
        let mut batch = self.batch();
        batch.remove(table, id);
        batch.commit();
    }

    pub fn replace<T: Record>(
        &self,
        table: &Table<T>,
        records: impl IntoIterator<Item = (String, T)>,
    ) {
        let mut batch = self.batch();
        batch.clear(table);
        for (id, value) in records {
            batch.put(table, &id, value);
        }
        batch.commit();
    }

    pub fn batch(&self) -> Batch {
        Batch::new(self.clone())
    }

    #[must_use]
    pub fn slot<T: Record>(&self, table: &Table<T>, id: &str) -> Slot<T> {
        Slot::new(self.clone(), *table, id)
    }

    #[must_use]
    pub fn blob(&self, area: &BlobArea, id: &str) -> Option<Vec<u8>> {
        let cache_key = CacheKey::new(area.name(), id);
        if let Some(pending) = self.shared.state.overlay().get_blob(&cache_key) {
            return pending.map(|bytes| bytes.to_vec());
        }

        let key = self.shared.keys.blob_key(area.name(), id);
        let sealed = self
            .read(|connection| store::blob(connection, area.name(), &key))
            .flatten()?;
        let Some(plain) = self.shared.keys.open_blob(area.name(), &key, &sealed) else {
            tracing::debug!(area = area.name(), "cached blob sealed with another key");
            self.send(vec![PendingWrite {
                change: Change::RemoveBlob(cache_key.clone()),
                op: Op::RemoveBlob { cache_key, key },
            }]);
            return None;
        };

        let used = self.next_use();
        self.send_untracked(Op::TouchBlob {
            area: area.name(),
            key,
            used,
        });
        Some(plain)
    }

    pub fn put_blob(&self, area: &BlobArea, id: &str, bytes: impl Into<Vec<u8>>) {
        let bytes: Vec<u8> = bytes.into();
        if bytes.len() as u64 > area.limit() {
            return;
        }
        let key = self.shared.keys.blob_key(area.name(), id);
        let sealed = match self.shared.keys.seal_blob(area.name(), &key, &bytes) {
            Ok(sealed) => sealed,
            Err(error) => {
                tracing::warn!(%error, "failed to seal cache blob");
                return;
            }
        };
        let used = self.next_use();
        let cache_key = CacheKey::new(area.name(), id);
        self.send(vec![PendingWrite {
            change: Change::PutBlob(cache_key.clone(), Arc::from(bytes)),
            op: Op::PutBlob {
                cache_key,
                key,
                sealed,
                used,
                limit: area.limit(),
            },
        }]);
    }

    pub fn remove_blob(&self, area: &BlobArea, id: &str) {
        let key = self.shared.keys.blob_key(area.name(), id);
        let cache_key = CacheKey::new(area.name(), id);
        self.send(vec![PendingWrite {
            change: Change::RemoveBlob(cache_key.clone()),
            op: Op::RemoveBlob { cache_key, key },
        }]);
    }

    pub fn flush(&self) -> Result<()> {
        let (sender, receiver) = mpsc::channel();
        self.shared
            .messages
            .send(Message::Flush(sender))
            .map_err(|_| Error::Stopped)?;
        receiver.recv().map_err(|_| Error::Stopped)?
    }

    pub fn clear(&self) -> Result<()> {
        self.wipe(false)
    }

    pub fn retire(&self) -> Result<()> {
        self.wipe(true)
    }

    fn wipe(&self, retiring: bool) -> Result<()> {
        {
            let mut overlay = self.shared.state.overlay();
            if retiring {
                self.shared.state.era.fetch_add(1, Ordering::SeqCst);
            }
            let era = self.shared.state.era.load(Ordering::SeqCst);
            let seq = self.shared.seq.fetch_add(1, Ordering::SeqCst) + 1;
            overlay.apply(Change::Wipe, seq);
            self.shared
                .messages
                .send(Message::Ops {
                    era,
                    ops: vec![(Op::Wipe, seq)],
                })
                .map_err(|_| Error::Stopped)?;
        }
        self.flush()
    }

    pub(crate) fn staged_put<T: Record>(
        &self,
        table: &Table<T>,
        id: &str,
        value: T,
    ) -> Option<PendingWrite> {
        let key = self.shared.keys.record_key(table.name(), id);
        let plain = match serde_json::to_vec(&(id, &value)) {
            Ok(plain) => plain,
            Err(error) => {
                tracing::warn!(table = table.name(), %error, "failed to encode cache record");
                return None;
            }
        };
        let sealed = match self.shared.keys.seal_record(table.name(), &key, &plain) {
            Ok(sealed) => sealed,
            Err(error) => {
                tracing::warn!(table = table.name(), %error, "failed to seal cache record");
                return None;
            }
        };
        let cache_key = CacheKey::new(table.name(), id);
        let value: Value = Arc::new(value);
        Some(PendingWrite {
            change: Change::Put(cache_key.clone(), value),
            op: Op::Put {
                cache_key,
                key,
                sealed,
                used: self.next_use(),
                limit: table.limit(),
            },
        })
    }

    pub(crate) fn staged_remove<T: Record>(&self, table: &Table<T>, id: &str) -> PendingWrite {
        let key = self.shared.keys.record_key(table.name(), id);
        let cache_key = CacheKey::new(table.name(), id);
        PendingWrite {
            change: Change::Remove(cache_key.clone()),
            op: Op::Remove { cache_key, key },
        }
    }

    pub(crate) fn staged_clear<T: Record>(&self, table: &Table<T>) -> PendingWrite {
        PendingWrite {
            change: Change::ClearTable(table.name()),
            op: Op::ClearTable(table.name()),
        }
    }

    pub(crate) fn send(&self, writes: Vec<PendingWrite>) {
        if writes.is_empty() {
            return;
        }
        let mut overlay = self.shared.state.overlay();
        let era = self.shared.state.era.load(Ordering::SeqCst);
        if era != self.era {
            tracing::debug!("dropped write from retired cache era");
            return;
        }
        let mut ops = Vec::with_capacity(writes.len());
        for PendingWrite { change, op } in writes {
            let seq = self.shared.seq.fetch_add(1, Ordering::SeqCst) + 1;
            overlay.apply(change, seq);
            ops.push((op, seq));
        }
        if self
            .shared
            .messages
            .send(Message::Ops { era, ops })
            .is_err()
        {
            tracing::warn!("cache writer stopped");
        }
    }

    fn touch<T>(&self, table: &Table<T>, key: Vec<u8>) {
        if table.limit().is_some() {
            let used = self.next_use();
            self.send_untracked(Op::TouchRecord {
                table: table.name(),
                key,
                used,
            });
        }
    }

    fn next_use(&self) -> u64 {
        self.shared.used.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn send_untracked(&self, op: Op) {
        let era = self.shared.state.era.load(Ordering::SeqCst);
        if era != self.era {
            return;
        }
        drop(self.shared.messages.send(Message::Ops {
            era,
            ops: vec![(op, 0)],
        }));
    }

    fn decrypt_record<T: Record>(
        &self,
        table: &Table<T>,
        key: &[u8],
        sealed: &[u8],
    ) -> Option<(String, T)> {
        let decrypted = self.open_record(table, key, sealed);
        if decrypted.is_none() {
            self.send_untracked(Op::Discard {
                table: table.name(),
                key: key.to_vec(),
                sealed: sealed.to_vec(),
            });
        }
        decrypted
    }

    fn open_record<T: Record>(
        &self,
        table: &Table<T>,
        key: &[u8],
        sealed: &[u8],
    ) -> Option<(String, T)> {
        let Some(plain) = self.shared.keys.open_record(table.name(), key, sealed) else {
            tracing::debug!(
                table = table.name(),
                "discarding cached record sealed with another key"
            );
            return None;
        };
        let (id, value): (String, T) = match serde_json::from_slice(&plain) {
            Ok(decoded) => decoded,
            Err(error) => {
                tracing::debug!(table = table.name(), %error, "discarding undecodable cached record");
                return None;
            }
        };
        (self.shared.keys.record_key(table.name(), &id) == key).then_some((id, value))
    }

    fn read<R>(&self, query: impl FnOnce(&Connection) -> Result<R>) -> Option<R> {
        let reader = self.shared.reader.lock();
        match query(&reader) {
            Ok(found) => Some(found),
            Err(error) => {
                tracing::warn!(%error, "failed to read cache");
                None
            }
        }
    }
}

impl fmt::Debug for Cache {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Cache")
            .field("path", &self.shared.location.path())
            .field("era", &self.era)
            .finish_non_exhaustive()
    }
}
