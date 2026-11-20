use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::time::Duration;

use rikka_compositor_protocols::wl_display;
use rikka_compositor_protocols::wl_fixes::{self, WlFixes};
use rikka_compositor_protocols::wl_registry::WlRegistry;
use rikka_wayland_wire::{ClientId, NewId, Object, Resource};

use crate::client::Client;
use crate::globals::{Entry, Globals};

const REMOVED_GLOBAL_GRACE: Duration = Duration::from_secs(5);

pub(crate) type Defer = Box<dyn Fn(Duration, Box<dyn FnOnce()>)>;

pub(crate) struct Core {
    clients: RefCell<Rc<[Client]>>,
    next_client: Cell<u64>,
    serial: Cell<u32>,
    globals: RefCell<Globals>,
    dirty: RefCell<Vec<ClientId>>,
    binds: RefCell<VecDeque<Object>>,
    wake: Box<dyn Fn()>,
    defer: Defer,
}

impl Core {
    pub(crate) fn new(wake: impl Fn() + 'static, defer: Defer) -> Self {
        Self {
            clients: RefCell::new(Rc::from([])),
            next_client: Cell::new(1),
            serial: Cell::new(0),
            globals: RefCell::new(Globals::default()),
            dirty: RefCell::new(Vec::new()),
            binds: RefCell::new(VecDeque::new()),
            wake: Box::new(wake),
            defer,
        }
    }

    pub(crate) fn allocate_client(&self) -> ClientId {
        let id = self.next_client.get();
        self.next_client.set(id + 1);
        ClientId::new(id)
    }

    pub(crate) fn admit(&self, client: Client) {
        let mut clients = self.clients.borrow_mut();
        let index = clients.partition_point(|known| known.id() < client.id());
        let mut admitted = clients.to_vec();
        admitted.insert(index, client);
        *clients = admitted.into();
    }

    pub(crate) fn dismiss(&self, id: ClientId) -> bool {
        let mut clients = self.clients.borrow_mut();
        let Ok(index) = clients.binary_search_by_key(&id, Client::id) else {
            return false;
        };
        let mut remaining = clients.to_vec();
        remaining.remove(index);
        *clients = remaining.into();
        true
    }

    pub(crate) fn client(&self, id: ClientId) -> Option<Client> {
        let clients = self.clients.borrow();
        let index = clients.binary_search_by_key(&id, Client::id).ok()?;
        Some(clients[index].clone())
    }

    pub(crate) fn clients(&self) -> Rc<[Client]> {
        self.clients.borrow().clone()
    }

    pub(crate) fn serial(&self) -> u32 {
        self.serial.get()
    }

    pub(crate) fn next_serial(&self) -> u32 {
        let serial = self.serial.get().wrapping_add(1);
        self.serial.set(serial);
        serial
    }

    pub(crate) fn mark_dirty(&self, id: ClientId) {
        self.dirty.borrow_mut().push(id);
        (self.wake)();
    }

    pub(crate) fn take_dirty(&self, into: &mut Vec<ClientId>) {
        std::mem::swap(&mut *self.dirty.borrow_mut(), into);
    }

    pub(crate) fn has_binds(&self) -> bool {
        !self.binds.borrow().is_empty()
    }

    pub(crate) fn next_bind(&self) -> Option<Object> {
        self.binds.borrow_mut().pop_front()
    }

    pub(crate) fn publish(&self, entry: Entry) -> u32 {
        let name = self.globals.borrow_mut().insert(entry.clone());
        let registries = self.globals.borrow().registries();
        for registry in registries {
            self.offer(name, &entry, &registry);
        }
        name
    }

    pub(crate) fn announce(&self, client: &Client, registry: WlRegistry) {
        self.globals.borrow_mut().add_registry(registry.clone());
        let published = self.globals.borrow().published();
        for (name, entry) in published {
            if entry.visible_to(client) {
                self.offer(name, &entry, &registry);
            }
        }
    }

    pub(crate) fn remove(self: &Rc<Self>, name: u32) {
        let holders = {
            let mut globals = self.globals.borrow_mut();
            let Some(entry) = globals.get_mut(name) else {
                return;
            };
            if entry.removed {
                return;
            }
            entry.removed = true;
            if entry.withdrawn() {
                globals.delete(name);
                return;
            }
            globals.holders(name)
        };
        for registry in holders {
            registry.global_remove(name);
        }
        let core = Rc::downgrade(self);
        (self.defer)(
            REMOVED_GLOBAL_GRACE,
            Box::new(move || {
                if let Some(core) = Weak::upgrade(&core) {
                    core.globals.borrow_mut().delete(name);
                }
            }),
        );
    }

    pub(crate) fn exists(&self, name: u32) -> bool {
        self.globals.borrow().get(name).is_some()
    }

    pub(crate) fn removed(&self, name: u32) -> bool {
        self.globals
            .borrow()
            .get(name)
            .is_none_or(|entry| entry.removed)
    }

    pub(crate) fn acknowledge(&self, fixes: &WlFixes, registry: &WlRegistry, name: u32) {
        let state = {
            let globals = self.globals.borrow();
            globals
                .get(name)
                .map(|entry| (entry.removed, globals.offered(name, registry)))
        };
        let complaint = match state {
            Some((true, true)) => {
                self.globals.borrow_mut().withdraw_offer(name, registry);
                return;
            }
            Some((false, true)) => format!("global {name} is not removed"),
            _ => format!("registry did not announce global {name}"),
        };
        fixes
            .object()
            .post_error(wl_fixes::Error::InvalidAckRemove.code(), &complaint);
    }

    pub(crate) fn forget_registry(&self, registry: &WlRegistry) {
        self.globals.borrow_mut().forget_registry(registry);
    }

    pub(crate) fn bind(&self, client: &Client, registry: &WlRegistry, name: u32, new: NewId<'_>) {
        let entry = self.globals.borrow().get(name).cloned();
        let invalid = |message: String| {
            registry
                .object()
                .post_error(wl_display::Error::InvalidObject.code(), &message);
        };
        let interface = new.interface;
        let entry = match entry {
            Some(entry) if entry.visible_to(client) => entry,
            _ => return invalid(format!("global {interface} ({name}) is unavailable")),
        };
        if entry.interface.name != interface {
            return invalid(format!(
                "invalid interface for global {name}: expected {}, got {interface}",
                entry.interface.name
            ));
        }
        if new.version == 0 {
            return invalid(format!(
                "invalid version for global {interface} ({name}): 0 is not a valid version"
            ));
        }
        if new.version > entry.version {
            return invalid(format!(
                "invalid version for global {interface} ({name}): expected at most {}, got {}",
                entry.version, new.version
            ));
        }
        match client.peer().insert(new.id, entry.interface, new.version) {
            Ok(object) => {
                if let Some(attach) = &entry.attach {
                    attach(&object);
                }
                self.binds.borrow_mut().push_back(object);
                (self.wake)();
            }
            Err(_) => client.post_display_error(
                wl_display::Error::InvalidObject,
                &format!("invalid new id {}", new.id),
            ),
        }
    }

    fn offer(&self, name: u32, entry: &Entry, registry: &WlRegistry) {
        if !registry.alive() {
            return;
        }
        let Some(client) = Client::from_resource(registry) else {
            return;
        };
        if !entry.visible_to(&client) {
            return;
        }
        self.globals.borrow_mut().offer(name, registry);
        registry.global(name, entry.interface.name, entry.version);
    }
}
