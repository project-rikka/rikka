use std::cell::RefCell;
use std::rc::Rc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::StateFile;

#[derive(Clone, Copy, Debug)]
pub struct RecentsConfig {
    pub file: &'static str,
    pub field: &'static str,
    pub capacity: usize,
}

#[derive(Debug)]
struct RecentsFile<T> {
    items: Vec<T>,
    config: RecentsConfig,
    file: Option<StateFile>,
}

impl<T> Drop for RecentsFile<T> {
    fn drop(&mut self) {
        if let Some(file) = &self.file {
            file.flush();
        }
    }
}

#[derive(Debug)]
pub struct Recents<T>(Rc<RefCell<RecentsFile<T>>>);

impl<T> Clone for Recents<T> {
    fn clone(&self) -> Recents<T> {
        Recents(Rc::clone(&self.0))
    }
}

impl<T: Clone + PartialEq + Serialize + DeserializeOwned> Recents<T> {
    #[must_use]
    pub fn read(config: RecentsConfig) -> Recents<T> {
        let file = StateFile::state(config.file);
        let mut items: Vec<T> = file
            .as_ref()
            .map(|file| read_items(file, config.field))
            .unwrap_or_default();
        items.truncate(config.capacity);
        Recents(Rc::new(RefCell::new(RecentsFile {
            items,
            config,
            file,
        })))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.borrow().items.is_empty()
    }

    #[must_use]
    pub fn items(&self) -> Vec<T> {
        self.0.borrow().items.clone()
    }

    pub fn record(&self, item: T) {
        self.change(|items, capacity| {
            items.retain(|recent| *recent != item);
            items.insert(0, item);
            items.truncate(capacity);
        });
    }

    pub fn remove(&self, removed: &[T]) {
        self.change(|items, _| items.retain(|recent| !removed.contains(recent)));
    }

    pub fn clear(&self) {
        self.change(|items, _| items.clear());
    }

    fn change(&self, edit: impl FnOnce(&mut Vec<T>, usize)) {
        let mut file = self.0.borrow_mut();
        let capacity = file.config.capacity;
        edit(&mut file.items, capacity);
        let Some(state) = &file.file else {
            return;
        };
        let items = match serde_json::to_value(&file.items) {
            Ok(items) => items,
            Err(error) => {
                tracing::warn!(%error, file = file.config.file, "failed to encode recent items");
                return;
            }
        };
        let document = Value::Object(Map::from_iter([(file.config.field.to_owned(), items)]));
        state.save(document.to_string());
    }
}

fn read_items<T: DeserializeOwned>(file: &StateFile, field: &str) -> Vec<T> {
    let mut document = file.load_json::<Map<String, Value>>();
    let Some(items) = document.remove(field) else {
        return Vec::new();
    };
    serde_json::from_value(items).unwrap_or_else(|error| {
        tracing::warn!(%error, path = %file.path().display(), "ignoring malformed recent items");
        Vec::new()
    })
}
