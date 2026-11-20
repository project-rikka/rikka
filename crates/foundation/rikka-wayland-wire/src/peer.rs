use std::any::Any;
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::fmt;
use std::rc::{Rc, Weak};

use crate::connection::Connection;
use crate::disconnect::Disconnect;
use crate::error::{Result, WireError};
use crate::ids::{ClientId, DISPLAY_ID};
use crate::interface::Interface;
use crate::object::Object;
use crate::table::{ObjectTable, Objects};

const ERROR_EVENT: u16 = 0;
const ERROR_MESSAGE_LIMIT: usize = 1024;

pub struct Peer {
    id: ClientId,
    connection: Connection,
    table: RefCell<ObjectTable>,
    open: Cell<bool>,
    closing: RefCell<Option<Disconnect>>,
    dirty: Cell<bool>,
    deferred_destroys: RefCell<Vec<Object>>,
    waker: RefCell<Option<Box<dyn Fn()>>>,
    owner: RefCell<Option<Weak<dyn Any>>>,
}

impl Peer {
    pub fn new(id: ClientId, connection: Connection) -> Rc<Self> {
        Rc::new(Self {
            id,
            connection,
            table: RefCell::new(ObjectTable::new()),
            open: Cell::new(true),
            closing: RefCell::new(None),
            dirty: Cell::new(false),
            deferred_destroys: RefCell::new(Vec::new()),
            waker: RefCell::new(None),
            owner: RefCell::new(None),
        })
    }

    pub fn id(&self) -> ClientId {
        self.id
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn set_waker(&self, waker: impl Fn() + 'static) {
        *self.waker.borrow_mut() = Some(Box::new(waker));
    }

    pub fn set_owner(&self, owner: Weak<dyn Any>) {
        *self.owner.borrow_mut() = Some(owner);
    }

    pub fn owner(&self) -> Option<Rc<dyn Any>> {
        self.owner.borrow().as_ref()?.upgrade()
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn closing(&self) -> Option<Disconnect> {
        self.closing.borrow().clone()
    }

    pub fn close(&self, reason: Disconnect) {
        if !self.open.replace(false) {
            return;
        }
        *self.closing.borrow_mut() = Some(reason);
        self.touch();
    }

    pub fn insert(
        self: &Rc<Self>,
        id: u32,
        interface: &'static Interface,
        version: u32,
    ) -> Result<Object> {
        let generation = self.table_mut().insert_client(id, interface, version)?;
        Ok(Object::new(self, id, generation, interface, version))
    }

    pub fn allocate(
        self: &Rc<Self>,
        interface: &'static Interface,
        version: u32,
    ) -> Option<Object> {
        let (id, generation) = self.table_mut().insert_server(interface, version)?;
        Some(Object::new(self, id, generation, interface, version))
    }

    pub fn detached(self: &Rc<Self>, interface: &'static Interface, version: u32) -> Object {
        let generation = self.table_mut().next_generation();
        Object::new(self, 0, generation, interface, version)
    }

    pub fn object(self: &Rc<Self>, id: u32) -> Option<Object> {
        let table = self.table();
        let slot = table.live(id)?;
        Some(Object::new(
            self,
            id,
            slot.generation,
            slot.interface,
            slot.version,
        ))
    }

    pub fn objects(self: &Rc<Self>) -> Objects<'_> {
        Objects::new(self, self.table_mut())
    }

    pub fn retire(&self, object: &Object) -> bool {
        self.table_mut().retire(object.id(), object.generation())
    }

    pub fn free(&self, object: &Object) {
        self.table_mut().free(object.id(), object.generation());
    }

    pub fn retire_all(self: &Rc<Self>) -> Vec<Object> {
        self.deferred_destroys.borrow_mut().clear();
        self.table_mut()
            .retire_all()
            .into_iter()
            .map(|(id, slot)| Object::new(self, id, slot.generation, slot.interface, slot.version))
            .collect()
    }

    pub fn free_all(&self) {
        self.table_mut().clear();
    }

    pub fn post_error(&self, interface: &'static str, object: u32, code: u32, message: &str) {
        if !self.is_open() {
            return;
        }
        let message = &message[..message.floor_char_boundary(ERROR_MESSAGE_LIMIT)];
        let result = {
            let mut writer = self.connection.write(DISPLAY_ID, ERROR_EVENT);
            writer.object_id(object).uint(code).string(message);
            writer.finish()
        };
        tracing::debug!(
            client = self.id.raw(),
            "protocol error {code} on {interface}#{object}: {message}"
        );
        self.close(match result {
            Err(WireError::Backlog) => Disconnect::Backlog,
            _ => Disconnect::Protocol {
                interface,
                object,
                code,
                message: message.to_owned(),
            },
        });
    }

    pub fn defer_destroy(&self, object: Object) {
        self.deferred_destroys.borrow_mut().push(object);
        self.touch();
    }

    pub fn take_deferred_destroys(&self, into: &mut Vec<Object>) {
        std::mem::swap(&mut *self.deferred_destroys.borrow_mut(), into);
    }

    pub fn has_deferred_destroys(&self) -> bool {
        !self.deferred_destroys.borrow().is_empty()
    }

    pub fn clean(&self) {
        self.dirty.set(false);
    }

    pub(crate) fn touch(&self) {
        if self.dirty.replace(true) {
            return;
        }
        if let Some(waker) = self.waker.borrow().as_ref() {
            waker();
        }
    }

    pub(crate) fn table(&self) -> Ref<'_, ObjectTable> {
        self.table.borrow()
    }

    pub(crate) fn table_mut(&self) -> RefMut<'_, ObjectTable> {
        self.table.borrow_mut()
    }
}

impl fmt::Debug for Peer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Peer")
            .field("id", &self.id)
            .field("open", &self.open.get())
            .field("dirty", &self.dirty.get())
            .finish_non_exhaustive()
    }
}
