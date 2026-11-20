use std::fmt;
use std::marker::PhantomData;

use serde::Serialize;
use serde::de::DeserializeOwned;

pub const SINGLETON_ID: &str = "";

pub trait Record: Serialize + DeserializeOwned + Clone + Send + Sync + 'static {}

impl<T> Record for T where T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static {}

pub struct Table<T> {
    name: &'static str,
    limit: Option<usize>,
    kind: PhantomData<fn() -> T>,
}

impl<T> Table<T> {
    #[must_use]
    pub const fn new(name: &'static str) -> Table<T> {
        Table {
            name,
            limit: None,
            kind: PhantomData,
        }
    }

    #[must_use]
    pub const fn bounded(name: &'static str, max_records: usize) -> Table<T> {
        Table {
            name,
            limit: Some(max_records),
            kind: PhantomData,
        }
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub const fn limit(&self) -> Option<usize> {
        self.limit
    }
}

impl<T> Clone for Table<T> {
    fn clone(&self) -> Table<T> {
        *self
    }
}

impl<T> Copy for Table<T> {}

impl<T> fmt::Debug for Table<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Table")
            .field("name", &self.name)
            .field("limit", &self.limit)
            .finish()
    }
}
