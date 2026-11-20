use rikka_sync::Mutex;

use crate::id::ItemId;

#[derive(Debug, Default)]
pub(crate) struct Registry {
    items: Mutex<Vec<ItemId>>,
}

impl Registry {
    pub(crate) fn add(&self, item: ItemId) -> bool {
        let mut items = self.locked();
        let fresh = !items.contains(&item);
        if fresh {
            items.push(item);
        }
        fresh
    }

    pub(crate) fn drop_owner(&self, service: &str) -> Vec<ItemId> {
        let mut items = self.locked();
        let (gone, left) = std::mem::take(&mut *items)
            .into_iter()
            .partition(|item| item.service == service);
        *items = left;
        gone
    }

    pub(crate) fn items(&self) -> Vec<ItemId> {
        self.locked().clone()
    }

    fn locked(&self) -> rikka_sync::MutexGuard<'_, Vec<ItemId>> {
        self.items.lock()
    }
}
