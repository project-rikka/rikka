use std::future::Future;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::entry::{Entry, Found, Secret};
use crate::error::Result;

#[derive(Clone, Debug)]
pub struct Secrets(crate::Secrets);

impl Secrets {
    pub fn shared() -> Result<Secrets> {
        block_on(crate::Secrets::shared()).map(Secrets)
    }

    pub fn store(&self, entry: &Entry, label: &str, secret: &[u8]) -> Result<()> {
        block_on(self.0.store(entry, label, secret))
    }

    pub fn store_json<T: Serialize>(&self, entry: &Entry, label: &str, value: &T) -> Result<()> {
        block_on(self.0.store_json(entry, label, value))
    }

    pub fn contains(&self, entry: &Entry) -> Result<bool> {
        block_on(self.0.contains(entry))
    }

    pub fn lookup(&self, entry: &Entry) -> Result<Option<Secret>> {
        block_on(self.0.lookup(entry))
    }

    pub fn lookup_json<T: DeserializeOwned>(&self, entry: &Entry) -> Result<Option<T>> {
        block_on(self.0.lookup_json(entry))
    }

    pub fn search(&self, entry: &Entry) -> Result<Vec<Found>> {
        block_on(self.0.search(entry))
    }

    pub fn remove(&self, entry: &Entry) -> Result<usize> {
        block_on(self.0.remove(entry))
    }
}

#[cfg(unix)]
fn block_on<F: Future>(future: F) -> F::Output {
    zbus::block_on(future)
}

#[cfg(windows)]
fn block_on<F: Future>(future: F) -> F::Output {
    crate::windows::block_on(future)
}
