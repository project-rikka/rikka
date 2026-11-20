use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::disconnect::Disconnect;
use crate::error::WireError;
use crate::object::Object;
use crate::writer::Writer;

pub struct EventWriter<'o> {
    writer: Writer<'o>,
    object: &'o Object,
    destructor: bool,
}

impl<'o> EventWriter<'o> {
    #[inline(never)]
    pub(crate) fn begin(object: &'o Object, opcode: u16) -> Option<Self> {
        let spec = object.interface().events.get(usize::from(opcode))?;
        if object.version() < spec.since || !object.alive() {
            return None;
        }
        let writer = object
            .peer()
            .connection()
            .write(object.id(), opcode)
            .owned_by(object.client());
        Some(Self {
            writer,
            object,
            destructor: spec.destructor,
        })
    }

    #[inline(never)]
    pub fn finish(self) {
        let Self {
            writer,
            object,
            destructor,
        } = self;
        let opcode = writer.opcode();
        match writer.finish() {
            Ok(()) => object.peer().touch(),
            Err(error) => dropped(object, opcode, error),
        }
        if destructor {
            object.expire();
            object.peer().defer_destroy(object.clone());
        }
    }
}

#[cold]
#[inline(never)]
fn dropped(object: &Object, opcode: u16, error: WireError) {
    match error {
        WireError::Backlog => object.peer().close(Disconnect::Backlog),
        error => tracing::warn!(
            client = object.client().raw(),
            "dropped {}#{} event {opcode}: {error}",
            object.interface().name,
            object.id()
        ),
    }
}

impl<'o> Deref for EventWriter<'o> {
    type Target = Writer<'o>;

    fn deref(&self) -> &Self::Target {
        &self.writer
    }
}

impl DerefMut for EventWriter<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.writer
    }
}

impl fmt::Debug for EventWriter<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EventWriter")
            .field("object", self.object)
            .field("writer", &self.writer)
            .finish()
    }
}
