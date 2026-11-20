use std::fmt;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

use rikka_wayland_wire::{Bindable, Interface, Key, Object};

use crate::client::Client;
use crate::display::DisplayHandle;
use crate::globals::{Attach, Entry, Filter};
use crate::shared::Core;

#[derive(Clone)]
pub struct Global {
    core: Weak<Core>,
    name: u32,
    interface: &'static Interface,
    version: u32,
}

impl Global {
    pub fn name(&self) -> u32 {
        self.name
    }

    pub fn interface(&self) -> &'static Interface {
        self.interface
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn remove(&self) {
        if let Some(core) = self.core.upgrade() {
            core.remove(self.name);
        }
    }

    pub fn removed(&self) -> bool {
        self.core
            .upgrade()
            .is_none_or(|core| core.removed(self.name))
    }

    pub fn bindable(&self) -> bool {
        self.core
            .upgrade()
            .is_some_and(|core| core.exists(self.name))
    }
}

impl PartialEq for Global {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && Weak::ptr_eq(&self.core, &other.core)
    }
}

impl Eq for Global {}

impl fmt::Debug for Global {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Global")
            .field("name", &self.name)
            .field("interface", &self.interface.name)
            .field("version", &self.version)
            .finish()
    }
}

#[must_use]
pub struct GlobalBuilder<'d, S: 'static, R> {
    display: &'d DisplayHandle<S>,
    version: u32,
    filter: Option<Filter>,
    attach: Option<Attach>,
    resource: PhantomData<fn() -> R>,
}

impl<'d, S: 'static, R: Bindable> GlobalBuilder<'d, S, R> {
    pub(crate) fn new(display: &'d DisplayHandle<S>) -> Self {
        Self {
            display,
            version: R::INTERFACE.version,
            filter: None,
            attach: None,
            resource: PhantomData,
        }
    }

    pub fn only(mut self, filter: impl Fn(&Client) -> bool + 'static) -> Self {
        let filter: Filter = match self.filter.take() {
            Some(earlier) => Rc::new(move |client: &Client| earlier(client) && filter(client)),
            None => Rc::new(filter),
        };
        self.filter = Some(filter);
        self
    }

    pub fn version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    pub fn privileged(self) -> Self {
        self.only(Client::privileged)
    }

    pub fn key<K: Key>(self, key: K) -> Self {
        self.on_bind(move |resource: &R| resource.object().set_key(key))
    }

    pub fn on_bind(mut self, hook: impl Fn(&R) + 'static) -> Self {
        let earlier = self.attach.take();
        self.attach = Some(Rc::new(move |object: &Object| {
            if let Some(earlier) = &earlier {
                earlier(object);
            }
            if let Some(resource) = R::from_object(object.clone()) {
                hook(&resource);
            }
        }));
        self
    }

    pub fn publish(self) -> Global {
        let interface = R::INTERFACE;
        assert!(
            (1..=interface.version).contains(&self.version),
            "{} global version {} is outside 1..={}",
            interface.name,
            self.version,
            interface.version
        );
        assert!(
            self.display.serves(interface),
            "{} is published as a global but not served",
            interface.name
        );
        let core = self.display.core();
        let name = core.publish(Entry::new(
            interface,
            self.version,
            self.filter,
            self.attach,
        ));
        Global {
            core: Rc::downgrade(core),
            name,
            interface,
            version: self.version,
        }
    }
}

impl<S: 'static, R> fmt::Debug for GlobalBuilder<'_, S, R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GlobalBuilder")
            .field("version", &self.version)
            .field("filtered", &self.filter.is_some())
            .finish_non_exhaustive()
    }
}
