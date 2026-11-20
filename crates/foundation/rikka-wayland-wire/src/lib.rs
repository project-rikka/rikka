pub mod client;
mod connection;
mod disconnect;
mod error;
mod event;
mod ids;
mod incoming;
mod interface;
mod key;
mod message;
mod object;
mod outgoing;
mod peer;
mod resource;
mod serial;
mod table;
mod writer;

#[doc(hidden)]
pub use slotmap as __slotmap;

pub use connection::{Connection, Fill, INCOMING_INITIAL, INCOMING_LIMIT};
pub use disconnect::Disconnect;
pub use error::{Result, WireError};
pub use event::EventWriter;
pub use ids::{
    CLIENT_ID_MAX, ClientId, DISPLAY_ID, IdPool, MAX_FDS_PER_MESSAGE, MAX_MESSAGE_SIZE,
    SERVER_ID_START, is_client_id,
};
pub use interface::{ArgType, Interface, MessageSpec, Signature};
pub use key::Key;
pub use message::{Arg, Args, Header, Message, NewId, Word};
pub use object::Object;
pub use outgoing::{Flush, OUTGOING_LIMIT};
pub use peer::Peer;
pub use resource::{Bindable, Resource, UnknownValue, raw_value};
pub use serial::serial_not_before;
pub use table::Objects;
pub use writer::{Fd, Writer};
