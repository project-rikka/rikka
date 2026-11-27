use std::collections::VecDeque;

use rikka_sync::Mutex;

#[derive(Debug)]
pub(crate) struct Memo<K, V> {
    entries: Mutex<VecDeque<(K, V)>>,
    capacity: usize,
}

impl<K: PartialEq, V: Clone> Memo<K, V> {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    pub(crate) fn get(&self, key: &K) -> Option<V> {
        let mut entries = self.entries.lock();
        let at = entries.iter().position(|(known, _)| known == key)?;
        let entry = entries.remove(at)?;
        let value = entry.1.clone();
        entries.push_front(entry);
        Some(value)
    }

    pub(crate) fn insert(&self, key: K, value: V) {
        let mut entries = self.entries.lock();
        entries.retain(|(known, _)| *known != key);
        entries.push_front((key, value));
        entries.truncate(self.capacity);
    }

    pub(crate) fn get_or_try<E>(
        &self,
        key: K,
        make: impl FnOnce() -> Result<V, E>,
    ) -> Result<V, E> {
        if let Some(value) = self.get(&key) {
            return Ok(value);
        }
        let value = make()?;
        self.insert(key, value.clone());
        Ok(value)
    }
}
