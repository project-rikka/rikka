#![recursion_limit = "256"]

mod ask;
mod body;
mod draft;
mod error;
mod folder;
mod link;
mod mailbox;
mod opened;
mod ops;
mod person;
mod plain;
mod prefetch;
mod row;
mod selection;
mod thread_cache;

pub use ask::{Ask, PAGE};
pub use body::{Body, Markdown, decode};
pub use draft::{Answering, Attaching, Draft, addresses, quoted};
pub use error::{Error, Result};
pub use folder::{Folder, Kind, arranged};
pub use link::Link;
pub use mailbox::{Invalidation, Mailbox};
pub use opened::{MessageAttachment, OpenedMessage, OpenedThread, Verification};
pub use ops::{
    Listing, attachment, cached_folders, cached_page, delete, discard_draft, folders, mark_read,
    move_rows, page, read, save_draft, send, set_label, set_starred, trash,
};
pub use person::Person;
pub use plain::Plain;
pub use prefetch::{prefetch, prefetch_candidates};
pub use row::{Row, RowId};
pub use thread_cache::ThreadCache;
