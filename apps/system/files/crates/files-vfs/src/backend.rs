use std::fmt::Debug;

use files_core::{Entry, Location};
use rikka_file_operations::{Action, Store};

use crate::error::{Error, Result};
use crate::watch::{Watch, WatchSink};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub create: bool,
    pub rename: bool,
    pub delete: bool,
    pub trash: bool,
    pub restore: bool,
    pub empty: bool,
    pub watch: bool,
    pub write: bool,
}

impl Capabilities {
    #[must_use]
    pub const fn read_only() -> Self {
        Self {
            create: false,
            rename: false,
            delete: false,
            trash: false,
            restore: false,
            empty: false,
            watch: false,
            write: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Detail {
    #[default]
    Typed,
    Bare,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ListOptions {
    pub show_hidden: bool,
    pub detail: Detail,
}

impl ListOptions {
    #[must_use]
    pub const fn walk() -> Self {
        Self {
            show_hidden: true,
            detail: Detail::Bare,
        }
    }
}

pub trait Backend: Store<Location = Location, Item = Entry> + Debug {
    fn scheme(&self) -> &str;

    fn parent_of(&self, at: &Location) -> Option<Location> {
        at.parent()
    }

    fn browses(&self, _at: &Location) -> bool {
        false
    }

    fn browse(&self, at: &Location, _password: Option<&str>) -> Result<Location> {
        Err(Error::Unsupported {
            at: at.clone(),
            action: Action::Anything,
        })
    }

    fn list(&self, at: &Location, options: ListOptions) -> Result<Vec<Entry>>;

    fn capabilities(&self, at: &Location) -> Capabilities;

    fn create_file(&self, at: &Location) -> Result<()>;

    fn rename(&self, at: &Location, name: &str) -> Result<Location>;

    fn watch(&self, at: &Location, sink: WatchSink) -> Result<Watch>;
}
