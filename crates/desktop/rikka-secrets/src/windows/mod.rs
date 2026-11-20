mod credential;
mod target;

use std::future::Future;
use std::io;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake};
use std::thread::Thread;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;
use zeroize::Zeroizing;

use self::credential::{LARGEST_BLOB, Stored, Written};
use crate::entry::{Attributes, Entry, Found, ItemId, Secret, StoredItem};
use crate::error::{Error, Result};

#[derive(Clone, Debug)]
pub struct Secrets {
    _shared: (),
}

impl Secrets {
    pub async fn shared() -> Result<Secrets> {
        Ok(Secrets { _shared: () })
    }

    pub async fn unlocked() -> Result<Secrets> {
        Secrets::shared().await
    }

    pub async fn connect() -> Result<Secrets> {
        Secrets::shared().await
    }

    pub async fn unlock(&self) -> Result<()> {
        Ok(())
    }

    pub async fn store(&self, entry: &Entry, label: &str, secret: &[u8]) -> Result<()> {
        store(entry, label, secret)
    }

    pub async fn store_json<T: Serialize>(
        &self,
        entry: &Entry,
        label: &str,
        value: &T,
    ) -> Result<()> {
        let encoded = Zeroizing::new(serde_json::to_vec(value)?);
        store(entry, label, &encoded)
    }

    pub async fn contains(&self, entry: &Entry) -> Result<bool> {
        Ok(!matching(entry)?.is_empty())
    }

    pub async fn lookup(&self, entry: &Entry) -> Result<Option<Secret>> {
        match matching(entry)?.into_iter().next() {
            Some((stored, _)) => Ok(Some(Secret::new(whole(&stored)?))),
            None => Ok(None),
        }
    }

    pub async fn lookup_json<T: DeserializeOwned>(&self, entry: &Entry) -> Result<Option<T>> {
        match self.lookup(entry).await? {
            Some(secret) => Ok(Some(serde_json::from_slice(secret.expose())?)),
            None => Ok(None),
        }
    }

    pub async fn search(&self, entry: &Entry) -> Result<Vec<Found>> {
        matching(entry)?
            .into_iter()
            .map(|(stored, attributes)| {
                Ok(Found {
                    attributes,
                    secret: Secret::new(whole(&stored)?),
                })
            })
            .collect()
    }

    pub async fn remove(&self, entry: &Entry) -> Result<usize> {
        let found = matching(entry)?;
        for (stored, _) in &found {
            erase(stored)?;
        }
        Ok(found.len())
    }

    pub async fn items(&self) -> Result<Vec<StoredItem>> {
        Ok(credential::enumerate(target::everything())
            .map_err(Error::Credentials)?
            .into_iter()
            .filter_map(|stored| {
                let attributes = target::attributes(&stored.target)?;
                Some(StoredItem {
                    label: stored.label,
                    attributes,
                    created: stored.created.unwrap_or(stored.written),
                    modified: stored.written,
                    id: ItemId(stored.target),
                })
            })
            .collect())
    }

    pub async fn delete(&self, item: &ItemId) -> Result<()> {
        match credential::read(&item.0).map_err(Error::Credentials)? {
            Some(stored) => erase(&stored),
            None => Ok(()),
        }
    }
}

fn store(entry: &Entry, label: &str, secret: &[u8]) -> Result<()> {
    let target = target::of(entry.attributes());
    let created = match credential::read(&target).map_err(Error::Credentials)? {
        Some(stored) => stored.created,
        None => None,
    }
    .unwrap_or_else(now);
    let chunks: Vec<&[u8]> = match secret.is_empty() {
        true => vec![secret],
        false => secret.chunks(LARGEST_BLOB).collect(),
    };
    for (index, chunk) in chunks.iter().enumerate().skip(1) {
        credential::write(&Written {
            target: &target::part(&target, index),
            label,
            blob: chunk,
            parts: None,
            created: None,
        })
        .map_err(Error::Credentials)?;
    }
    credential::write(&Written {
        target: &target,
        label,
        blob: chunks[0],
        parts: Some(chunks.len()),
        created: Some(created),
    })
    .map_err(Error::Credentials)?;
    let mut stale = chunks.len();
    while credential::delete(&target::part(&target, stale)).map_err(Error::Credentials)? {
        stale += 1;
    }
    Ok(())
}

fn matching(entry: &Entry) -> Result<Vec<(Stored, Attributes)>> {
    let wanted = entry.attributes();
    Ok(credential::enumerate(&target::filter(wanted))
        .map_err(Error::Credentials)?
        .into_iter()
        .filter_map(|stored| {
            let attributes = target::attributes(&stored.target)?;
            let includes = wanted
                .iter()
                .all(|(name, value)| attributes.get(name) == Some(value));
            includes.then_some((stored, attributes))
        })
        .collect())
}

fn whole(stored: &Stored) -> Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(stored.blob.to_vec());
    for index in 1..stored.parts {
        let part = credential::read(&target::part(&stored.target, index))
            .map_err(Error::Credentials)?
            .ok_or_else(|| {
                Error::Credentials(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("part {index} of {} is missing", stored.target),
                ))
            })?;
        bytes.extend_from_slice(&part.blob);
    }
    Ok(bytes)
}

fn erase(stored: &Stored) -> Result<()> {
    for index in 1..stored.parts {
        credential::delete(&target::part(&stored.target, index)).map_err(Error::Credentials)?;
    }
    credential::delete(&stored.target).map_err(Error::Credentials)?;
    Ok(())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

struct Unpark(Thread);

impl Wake for Unpark {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let waker = Arc::new(Unpark(std::thread::current())).into();
    let mut context = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::park();
    }
}
