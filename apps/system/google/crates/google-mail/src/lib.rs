mod address;
mod compose;
mod error;
mod label;
mod mail;
mod message;
mod pictures;
mod reading;
mod words;

pub use address::Address;
pub use compose::{Enclosure, Outgoing};
pub use error::{Error, Result};
pub use label::{Label, Mailbox};
pub use mail::{Activity, Draft, Mail, Profile, Query, ThreadPage};
pub use message::{
    Attachment, DRAFT, IMPORTANT, INBOX, Message, SENT, SPAM, STARRED, Summary, TRASH, Thread,
    UNREAD,
};
pub use pictures::Pictures;
