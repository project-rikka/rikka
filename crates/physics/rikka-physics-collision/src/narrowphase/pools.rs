use rikka_physics_core::{PerWorker, Shared};

use crate::ContactManifold;

#[derive(Debug)]
pub struct ManifoldPools {
    pools: [PerWorker<Vec<ContactManifold>>; 2],
    current: usize,
    stamp: u32,
}

impl Default for ManifoldPools {
    fn default() -> Self {
        Self {
            pools: [
                PerWorker::new(1, |_| Vec::new()),
                PerWorker::new(1, |_| Vec::new()),
            ],
            current: 0,
            stamp: 0,
        }
    }
}

impl ManifoldPools {
    #[inline]
    pub fn stamp(&self) -> u32 {
        self.stamp
    }

    pub(crate) fn begin(&mut self, workers: usize) {
        if self.pools[0].len() != workers {
            self.pools = [
                PerWorker::new(workers, |_| Vec::new()),
                PerWorker::new(workers, |_| Vec::new()),
            ];
        }
        self.current ^= 1;
        self.stamp = self.stamp.wrapping_add(1);
        for pool in self.pools[self.current].iter_mut() {
            pool.clear();
        }
    }

    pub(crate) fn split<'a>(
        &'a mut self,
        previous: &mut Vec<PoolView>,
    ) -> &'a PerWorker<Vec<ContactManifold>> {
        let (low, high) = self.pools.split_at_mut(1);
        let (current, old) = if self.current == 0 {
            (&mut low[0], &mut high[0])
        } else {
            (&mut high[0], &mut low[0])
        };
        previous.clear();
        previous.extend(old.iter_mut().map(|pool| PoolView::new(pool)));
        current
    }

    pub fn extend_extras(&self, span: crate::ExtraManifolds, out: &mut Vec<ContactManifold>) {
        if span.count == 0 || span.stamp != self.stamp {
            return;
        }
        let pool = self.pools[self.current].local(span.worker);
        out.extend_from_slice(&pool[span.start as usize..(span.start + span.count) as usize]);
    }

    pub fn extra(&self, span: crate::ExtraManifolds, index: u32) -> Option<ContactManifold> {
        if span.stamp != self.stamp || index >= span.count {
            return None;
        }
        self.pools[self.current]
            .local(span.worker)
            .get((span.start + index) as usize)
            .copied()
    }

    pub fn extra_mut(
        &mut self,
        span: crate::ExtraManifolds,
        index: u32,
    ) -> Option<&mut ContactManifold> {
        if span.stamp != self.stamp || index >= span.count {
            return None;
        }
        self.pools[self.current]
            .get_mut(span.worker)
            .get_mut((span.start + index) as usize)
    }

    pub fn views(&mut self, out: &mut Vec<PoolView>) {
        out.clear();
        out.extend(
            self.pools[self.current]
                .iter_mut()
                .map(|pool| PoolView::new(pool)),
        );
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PoolView {
    ptr: *mut ContactManifold,
    len: usize,
}

unsafe impl Send for PoolView {}
unsafe impl Sync for PoolView {}

impl PoolView {
    fn new(pool: &mut [ContactManifold]) -> Self {
        Self {
            ptr: pool.as_mut_ptr(),
            len: pool.len(),
        }
    }

    #[inline(always)]
    pub unsafe fn shared<'a>(self) -> Shared<'a, ContactManifold> {
        unsafe { Shared::from_raw(self.ptr, self.len) }
    }
}
