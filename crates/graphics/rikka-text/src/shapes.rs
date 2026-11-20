use rikka_sync::{Mutex, MutexGuard};
use std::sync::{Arc, OnceLock};

use crate::lru_cache::LruCache;
use crate::shaped::{LineRequest, Shaped, ShapedLine};
use crate::shaping::TextShape;

const SHAPED_BUDGET: usize = 16 * 1024 * 1024;

const LINE_BUDGET: usize = 24 * 1024 * 1024;

const SHARDS: usize = 8;

pub(crate) struct ShardedLruCache<V> {
    shards: OnceLock<Vec<Mutex<LruCache<u64, V>>>>,
    budget: usize,
}

impl<V> ShardedLruCache<V> {
    pub(crate) const fn new(budget: usize) -> Self {
        Self {
            shards: OnceLock::new(),
            budget,
        }
    }

    pub(crate) fn shard(&self, key: u64) -> MutexGuard<'_, LruCache<u64, V>> {
        let shards = self.shards.get_or_init(|| {
            (0..SHARDS)
                .map(|_| Mutex::new(LruCache::new(self.budget / SHARDS)))
                .collect()
        });
        shards[key as usize % SHARDS].lock()
    }
}

static SHAPED_CACHE: ShardedLruCache<Arc<Shaped>> = ShardedLruCache::new(SHAPED_BUDGET);

static LINE_CACHE: ShardedLruCache<Arc<ShapedLine>> = ShardedLruCache::new(LINE_BUDGET);

#[must_use]
pub fn shaped(shape: TextShape<'_>) -> Arc<Shaped> {
    let key = shape.shape_digest();
    if let Some(found) = SHAPED_CACHE
        .shard(key)
        .get(key)
        .filter(|cached| cached.is(&shape))
    {
        return Arc::clone(found);
    }
    let shaped = Arc::new(Shaped::build(&shape, shape_line));
    SHAPED_CACHE
        .shard(key)
        .insert(key, Arc::clone(&shaped), shaped.bytes());
    shaped
}

fn shape_line(request: LineRequest<'_>) -> Arc<ShapedLine> {
    let key = request.digest();
    if let Some(found) = LINE_CACHE
        .shard(key)
        .get(key)
        .filter(|cached| cached.is(&request))
    {
        return Arc::clone(found);
    }
    let shaped = Arc::new(request.shape());
    LINE_CACHE
        .shard(key)
        .insert(key, Arc::clone(&shaped), shaped.bytes);
    shaped
}

const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<Shaped>();
    assert_send_sync::<crate::text_layout::TextLayout>();
    const fn assert_send<T: Send>() {}
    assert_send::<crate::glyph_raster::GlyphRaster>();
};
