use std::collections::BTreeMap;
use std::rc::Rc;

use rikka_compositor_protocols::wl_registry::WlRegistry;
use rikka_wayland_wire::{Interface, Object};
use rustc_hash::FxHashMap;

use crate::client::Client;

pub(crate) type Filter = Rc<dyn Fn(&Client) -> bool>;
pub(crate) type Attach = Rc<dyn Fn(&Object)>;

#[derive(Clone)]
pub(crate) struct Entry {
    pub(crate) interface: &'static Interface,
    pub(crate) version: u32,
    pub(crate) filter: Option<Filter>,
    pub(crate) attach: Option<Attach>,
    pub(crate) removed: bool,
    holders: usize,
}

impl Entry {
    pub(crate) fn new(
        interface: &'static Interface,
        version: u32,
        filter: Option<Filter>,
        attach: Option<Attach>,
    ) -> Self {
        Self {
            interface,
            version,
            filter,
            attach,
            removed: false,
            holders: 0,
        }
    }

    pub(crate) fn visible_to(&self, client: &Client) -> bool {
        self.filter.as_ref().is_none_or(|filter| filter(client))
    }

    pub(crate) fn withdrawn(&self) -> bool {
        self.removed && self.holders == 0
    }
}

#[derive(Default)]
pub(crate) struct Globals {
    entries: BTreeMap<u32, Entry>,
    next_name: u32,
    registries: FxHashMap<WlRegistry, Vec<u32>>,
}

impl Globals {
    pub(crate) fn insert(&mut self, entry: Entry) -> u32 {
        self.next_name += 1;
        self.entries.insert(self.next_name, entry);
        self.next_name
    }

    pub(crate) fn get(&self, name: u32) -> Option<&Entry> {
        self.entries.get(&name)
    }

    pub(crate) fn get_mut(&mut self, name: u32) -> Option<&mut Entry> {
        self.entries.get_mut(&name)
    }

    pub(crate) fn delete(&mut self, name: u32) {
        if self.entries.remove(&name).is_none() {
            return;
        }
        for offered in self.registries.values_mut() {
            if let Ok(index) = offered.binary_search(&name) {
                offered.remove(index);
            }
        }
    }

    pub(crate) fn published(&self) -> Vec<(u32, Entry)> {
        self.entries
            .iter()
            .filter(|(_, entry)| !entry.removed)
            .map(|(name, entry)| (*name, entry.clone()))
            .collect()
    }

    pub(crate) fn registries(&self) -> Vec<WlRegistry> {
        self.registries.keys().cloned().collect()
    }

    pub(crate) fn holders(&self, name: u32) -> Vec<WlRegistry> {
        self.registries
            .iter()
            .filter(|(_, offered)| offered.binary_search(&name).is_ok())
            .map(|(registry, _)| registry.clone())
            .collect()
    }

    pub(crate) fn offered(&self, name: u32, registry: &WlRegistry) -> bool {
        self.registries
            .get(registry)
            .is_some_and(|offered| offered.binary_search(&name).is_ok())
    }

    pub(crate) fn add_registry(&mut self, registry: WlRegistry) {
        self.registries.entry(registry).or_default();
    }

    pub(crate) fn offer(&mut self, name: u32, registry: &WlRegistry) {
        let (Some(entry), Some(offered)) = (
            self.entries.get_mut(&name),
            self.registries.get_mut(registry),
        ) else {
            return;
        };
        if let Err(index) = offered.binary_search(&name) {
            offered.insert(index, name);
            entry.holders += 1;
        }
    }

    pub(crate) fn withdraw_offer(&mut self, name: u32, registry: &WlRegistry) {
        let Some(offered) = self.registries.get_mut(registry) else {
            return;
        };
        if let Ok(index) = offered.binary_search(&name) {
            offered.remove(index);
            self.release(name);
        }
    }

    pub(crate) fn forget_registry(&mut self, registry: &WlRegistry) {
        let Some(offered) = self.registries.remove(registry) else {
            return;
        };
        for name in offered {
            self.release(name);
        }
    }

    fn release(&mut self, name: u32) {
        let Some(entry) = self.entries.get_mut(&name) else {
            return;
        };
        entry.holders = entry.holders.saturating_sub(1);
        if entry.withdrawn() {
            self.entries.remove(&name);
        }
    }
}
