use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use music_library::Library;
use rikka_app::prelude::*;

const CAPACITY: usize = 48;

type CachedPage = Arc<dyn Any + Send + Sync>;

#[derive(Default)]
struct Shown {
    pages: HashMap<u64, (u64, CachedPage)>,
    stamp: u64,
}

thread_local! {
    static SHOWN: RefCell<Shown> = RefCell::default();
}

#[derive(Clone)]
pub(crate) struct Shelf {
    pub(crate) library: Library,
    pub(crate) indexed: u64,
    pub(crate) listened: u64,
}

impl Shelf {
    pub(crate) fn read<T: Send + Sync + 'static>(
        &self,
        what: impl Hash,
        read: impl FnOnce(&Library) -> music_library::Result<T> + Send + 'static,
    ) -> Fetch<T> {
        self.fetch(what, None, read)
    }

    pub(crate) fn read_listening<T: Send + Sync + 'static>(
        &self,
        what: impl Hash,
        read: impl FnOnce(&Library) -> music_library::Result<T> + Send + 'static,
    ) -> Fetch<T> {
        self.fetch(what, Some(self.listened), read)
    }

    fn fetch<T: Send + Sync + 'static>(
        &self,
        what: impl Hash,
        listened: Option<u64>,
        read: impl FnOnce(&Library) -> music_library::Result<T> + Send + 'static,
    ) -> Fetch<T> {
        let mut hasher = DefaultHasher::new();
        what.hash(&mut hasher);
        std::any::type_name::<T>().hash(&mut hasher);
        let place = hasher.finish();
        let library = self.library.clone();
        let edition = (self.indexed, listened);
        let snapshot = fetch_blocking(("music-local", place, edition), move || read(&library));
        match snapshot {
            Fetch::Ready(value) => {
                insert(place, Arc::clone(&value) as CachedPage);
                Fetch::Ready(value)
            }
            Fetch::Loading => cached(place).map_or(Fetch::Loading, Fetch::Ready),
            failed @ Fetch::Failed(_) => failed,
        }
    }
}

fn insert(place: u64, value: CachedPage) {
    SHOWN.with(|shown| {
        let mut shown = shown.borrow_mut();
        shown.stamp += 1;
        let stamp = shown.stamp;
        shown.pages.insert(place, (stamp, value));
        if shown.pages.len() > CAPACITY
            && let Some(oldest) = shown
                .pages
                .iter()
                .min_by_key(|(_, (stamp, _))| *stamp)
                .map(|(place, _)| *place)
        {
            shown.pages.remove(&oldest);
        }
    });
}

fn cached<T: Send + Sync + 'static>(place: u64) -> Option<Arc<T>> {
    SHOWN.with(|shown| {
        let page = shown
            .borrow()
            .pages
            .get(&place)
            .map(|(_, page)| Arc::clone(page))?;
        page.downcast::<T>().ok()
    })
}
