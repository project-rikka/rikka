use std::cell::Cell;

thread_local! {
    static CURRENT: Cell<Option<(usize, u32)>> = const { Cell::new(None) };
}

pub(super) struct CurrentWorker {
    previous: Option<(usize, u32)>,
}

impl CurrentWorker {
    pub(super) fn enter(pool: usize, worker: u32) -> Self {
        Self {
            previous: CURRENT.replace(Some((pool, worker))),
        }
    }

    pub(super) fn index_in(pool: usize) -> Option<u32> {
        CURRENT
            .get()
            .and_then(|(current, worker)| (current == pool).then_some(worker))
    }
}

impl Drop for CurrentWorker {
    fn drop(&mut self) {
        CURRENT.set(self.previous);
    }
}
