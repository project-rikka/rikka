use std::ops::RangeInclusive;

use rikka_wayland_wire::client::Proxy;

use crate::wl_registry::{Event, WlRegistry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Global {
    pub name: u32,
    pub interface: Box<str>,
    pub version: u32,
}

#[derive(Debug)]
pub struct Globals {
    registry: WlRegistry,
    known: Vec<Global>,
}

impl Globals {
    pub fn new(registry: WlRegistry) -> Self {
        Self {
            registry,
            known: Vec::new(),
        }
    }

    pub fn registry(&self) -> &WlRegistry {
        &self.registry
    }

    pub fn event(&mut self, event: Event) {
        match event {
            Event::Global {
                name,
                interface,
                version,
            } => self.known.push(Global {
                name,
                interface: interface.into(),
                version,
            }),
            Event::GlobalRemove { name } => self.known.retain(|global| global.name != name),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Global> {
        self.known.iter()
    }

    pub fn of<P: Proxy>(&self) -> impl Iterator<Item = &Global> {
        self.known
            .iter()
            .filter(|global| &*global.interface == P::INTERFACE.name)
    }

    pub fn bind<P: Proxy>(&self, versions: RangeInclusive<u32>) -> Option<P> {
        self.of::<P>()
            .find(|global| global.version >= *versions.start())
            .map(|global| self.bind_global(global, *versions.end()))
    }

    pub fn bind_all<P: Proxy>(&self, version: u32) -> Vec<P> {
        self.of::<P>()
            .map(|global| self.bind_global(global, version))
            .collect()
    }

    pub fn bind_global<P: Proxy>(&self, global: &Global, version: u32) -> P {
        self.registry.bind(global.name, version.min(global.version))
    }
}
