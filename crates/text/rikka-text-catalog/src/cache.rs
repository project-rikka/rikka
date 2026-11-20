use rikka_sync::RwLock;
use std::borrow::Borrow;
use std::hash::{BuildHasher, Hash};

use rustc_hash::{FxBuildHasher, FxHashMap};

const SHARD_BITS: u32 = 4;
const SHARDS: usize = 1 << SHARD_BITS;
const SHARD_CAPACITY: usize = 8192;
const MULTIPLIER: u64 = 0x517C_C1B7_2722_0A95;

pub(crate) struct ShardedCache<K, V> {
    shards: Box<[RwLock<FxHashMap<K, V>>]>,
}

impl<K: Hash + Eq, V: Copy> ShardedCache<K, V> {
    pub(crate) fn new() -> Self {
        Self {
            shards: (0..SHARDS).map(|_| RwLock::default()).collect(),
        }
    }

    fn shard<Q: Hash + ?Sized>(&self, key: &Q) -> &RwLock<FxHashMap<K, V>> {
        let hash = FxBuildHasher.hash_one(key);
        let index = (hash.wrapping_mul(MULTIPLIER) >> (64 - SHARD_BITS)) as usize;
        &self.shards[index]
    }

    #[inline]
    pub(crate) fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.shard(key).read().get(key).copied()
    }

    pub(crate) fn insert(&self, key: K, value: V) {
        let mut shard = self.shard(&key).write();
        if shard.len() >= SHARD_CAPACITY {
            shard.clear();
        }
        shard.insert(key, value);
    }
}
