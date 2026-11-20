use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::private::write_private;
use crate::removal::Removal;
use crate::vault::Vault;

const BOOK: &str = "accounts.json";
const FOLDERS: &str = "accounts";
const PRIVATE_DIR: u32 = 0o700;
const ATTEMPTS: u32 = 16;

pub trait Record: Clone + PartialEq + Serialize + DeserializeOwned {
    const VAULT: Vault;

    fn key(&self) -> &str;

    fn is_same_account(&self, other: &Self) -> bool;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(bound(deserialize = "R: DeserializeOwned"))]
struct Book<R> {
    current: Option<String>,
    accounts: Vec<R>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Accounts<R> {
    root: PathBuf,
    book: Book<R>,
}

#[derive(Debug)]
pub struct PendingAccount {
    key: String,
    folder: PathBuf,
    vault: Vault,
    settled: bool,
}

#[derive(Debug)]
pub struct Committed<R> {
    pub saved: R,
    pub replaced: Option<Removal>,
}

impl PendingAccount {
    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub fn abandon(mut self) -> Removal {
        self.settled = true;
        Removal::start(self.vault, std::mem::take(&mut self.folder))
    }
}

impl Drop for PendingAccount {
    fn drop(&mut self) {
        if !self.settled {
            drop(Removal::start(self.vault, std::mem::take(&mut self.folder)));
        }
    }
}

impl<R: Record> Accounts<R> {
    pub fn open(root: impl Into<PathBuf>) -> Result<Accounts<R>> {
        let root = root.into();
        let path = root.join(BOOK);
        let book = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(Error::json(&path))?,
            Err(error) if error.kind() == ErrorKind::NotFound => Book {
                current: None,
                accounts: Vec::new(),
            },
            Err(error) => return Err(Error::io(&path)(error)),
        };
        Ok(Accounts { root, book })
    }

    #[must_use]
    pub fn all(&self) -> &[R] {
        &self.book.accounts
    }

    #[must_use]
    pub fn find(&self, key: &str) -> Option<&R> {
        self.book.accounts.iter().find(|saved| saved.key() == key)
    }

    #[must_use]
    pub fn current(&self) -> Option<&R> {
        self.book.current.as_deref().and_then(|key| self.find(key))
    }

    #[must_use]
    pub fn folder(&self, key: &str) -> PathBuf {
        self.root.join(FOLDERS).join(key)
    }

    pub fn reserve(&self) -> Result<PendingAccount> {
        let folders = self.root.join(FOLDERS);
        private_dir(true)
            .create(&folders)
            .map_err(Error::io(&folders))?;
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        for attempt in 0..ATTEMPTS {
            let key = format!("{:x}{:x}{attempt:x}", seed, std::process::id());
            let folder = folders.join(&key);
            match private_dir(false).create(&folder) {
                Ok(()) => {
                    return Ok(PendingAccount {
                        key,
                        folder,
                        vault: R::VAULT,
                        settled: false,
                    });
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(Error::io(&folder)(error)),
            }
        }
        Err(Error::NoFreeFolder(folders))
    }

    pub fn commit(
        &mut self,
        mut pending: PendingAccount,
        record: impl FnOnce(String) -> R,
    ) -> Result<Committed<R>> {
        let saved = record(pending.key.clone());
        let replaced = self
            .book
            .accounts
            .iter()
            .position(|known| known.is_same_account(&saved));
        let mut book = self.book.clone();
        let replaced = match replaced {
            Some(at) => Some(std::mem::replace(&mut book.accounts[at], saved.clone())),
            None => {
                book.accounts.push(saved.clone());
                None
            }
        };
        book.current = Some(saved.key().to_owned());
        self.write(&book)?;
        self.book = book;
        pending.settled = true;
        Ok(Committed {
            saved,
            replaced: replaced.map(|replaced| self.removal(replaced.key())),
        })
    }

    pub fn choose(&mut self, key: &str) -> Result<()> {
        if self.find(key).is_none() || self.book.current.as_deref() == Some(key) {
            return Ok(());
        }
        let mut book = self.book.clone();
        book.current = Some(key.to_owned());
        self.write(&book)?;
        self.book = book;
        Ok(())
    }

    pub fn update(&mut self, key: &str, change: impl FnOnce(&mut R)) -> Result<()> {
        let Some(at) = self
            .book
            .accounts
            .iter()
            .position(|known| known.key() == key)
        else {
            return Ok(());
        };
        let mut book = self.book.clone();
        change(&mut book.accounts[at]);
        if book.accounts[at] == self.book.accounts[at] {
            return Ok(());
        }
        self.write(&book)?;
        self.book = book;
        Ok(())
    }

    pub fn forget(&mut self, key: &str) -> Result<Option<Removal>> {
        if self.find(key).is_none() {
            return Ok(None);
        }
        let mut book = self.book.clone();
        book.accounts.retain(|known| known.key() != key);
        if book.current.as_deref() == Some(key) {
            book.current = book.accounts.first().map(|first| first.key().to_owned());
        }
        self.write(&book)?;
        self.book = book;
        Ok(Some(self.removal(key)))
    }

    fn removal(&self, key: &str) -> Removal {
        Removal::start(R::VAULT, self.folder(key))
    }

    fn write(&self, book: &Book<R>) -> Result<()> {
        private_dir(true)
            .create(&self.root)
            .map_err(Error::io(&self.root))?;
        let path = self.root.join(BOOK);
        let bytes = serde_json::to_vec_pretty(book).map_err(Error::json(&path))?;
        write_private(&path, &bytes)
    }
}

fn private_dir(recursive: bool) -> fs::DirBuilder {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(recursive).mode(PRIVATE_DIR);
    builder
}
