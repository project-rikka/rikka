use crate::fx::cache::BackdropCache;
use crate::fx::recipe::{Plan, StaleCorners};

use super::FxExecutor;
use super::effect_output::{EffectOutput, cached_output};

impl FxExecutor {
    pub(crate) fn retain_backdrops(&mut self, keys: &[u64]) {
        self.retained_keys.clear();
        self.retained_keys.extend_from_slice(keys);
    }

    pub(crate) fn keep_alive(&mut self, keys: impl IntoIterator<Item = u64>) {
        self.alive_keys.clear();
        self.alive_keys.extend(keys);
    }

    pub(crate) fn invalidate(&mut self, key: u64) {
        self.cache.invalidate(key);
    }

    pub(crate) fn matches(&self, plan: &Plan, cache: &BackdropCache) -> bool {
        cache.stale.is_empty()
            && self
                .stale_corners(plan, cache)
                .is_some_and(|reshaped| reshaped.is_empty())
    }

    pub(crate) fn stale_corners(&self, plan: &Plan, cache: &BackdropCache) -> Option<StaleCorners> {
        let scale = plan.scale()?;
        let cached = self
            .cache
            .get(cache.key)
            .filter(|cached| cached.matches(plan.region, (scale, plan.signature), self.format))?;
        Some(plan.reshaped(cached.rounded))
    }

    pub(crate) fn reusable(&mut self, plan: &Plan, cache: &BackdropCache) -> Option<EffectOutput> {
        if !self.matches(plan, cache) {
            return None;
        }
        let cached = self.cache.get_mut(cache.key)?;
        cached.mark_used(self.frame_now);
        Some(cached_output(cached, plan.region))
    }
}
