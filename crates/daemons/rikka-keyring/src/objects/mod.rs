mod collection;
mod item;
mod keyring;
mod prompt;
mod service;
mod session;

pub(crate) use collection::CollectionObject;
pub(crate) use item::ItemObject;
pub(crate) use keyring::KeyringObject;
pub(crate) use prompt::PromptObject;
pub(crate) use service::ServiceObject;
pub(crate) use session::SessionObject;

use zbus::message::Header;

use crate::fault::{Answer, Fault};

fn caller(header: &Header<'_>) -> Answer<String> {
    header
        .sender()
        .map(ToString::to_string)
        .ok_or_else(|| Fault::denied("the call has no sender"))
}
