use std::fmt;
use std::hash::Hash;

use crate::client::handle::Handle;
use crate::client::object_id::ObjectId;
use crate::client::packet::Resolved;
use crate::error::Result;
use crate::interface::Interface;
use crate::message::Args;

pub trait Proxy: Clone + Eq + Hash + fmt::Debug + Sized + Send + Sync + 'static {
    type Event: fmt::Debug + Send + 'static;

    const INTERFACE: &'static Interface;

    fn from_handle(handle: Handle) -> Option<Self>;

    fn handle(&self) -> &Handle;

    fn id(&self) -> u32 {
        self.handle().id()
    }

    fn object_id(&self) -> ObjectId {
        self.handle().object_id()
    }

    fn version(&self) -> u32 {
        self.handle().version()
    }

    fn alive(&self) -> bool {
        self.handle().alive()
    }

    fn decode(
        parent: &Handle,
        opcode: u16,
        args: &mut Args<'_>,
        objects: &mut Resolved<'_>,
    ) -> Result<Self::Event>;
}
