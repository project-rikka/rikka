use std::any::{Any, TypeId};

use rustc_hash::FxHashMap;

use crate::coordinates::Conversion;
use crate::material::MaterialAlias;
use crate::model::Model;
use crate::rig::Rig;
use crate::rig_facts::RigFacts;

pub trait Extension: Any + Clone + Send + Sync {
    const NAME: &'static str;

    fn convert(&mut self, conversion: &Conversion) {
        let _ = conversion;
    }

    fn rig(&self, model: &Model) -> Option<Box<dyn Rig>> {
        let _ = model;
        None
    }

    fn describe_rig(&self, facts: &mut RigFacts) {
        let _ = facts;
    }

    fn alias_materials(&self, aliases: &mut Vec<MaterialAlias>) {
        let _ = aliases;
    }
}

type BoxedValue = Box<dyn Any + Send + Sync>;

struct Entry {
    name: &'static str,
    value: BoxedValue,
    copy: fn(&(dyn Any + Send + Sync)) -> BoxedValue,
    turn: fn(&mut (dyn Any + Send + Sync), &Conversion),
    rig: fn(&(dyn Any + Send + Sync), &Model) -> Option<Box<dyn Rig>>,
    describe: fn(&(dyn Any + Send + Sync), &mut RigFacts),
    alias: fn(&(dyn Any + Send + Sync), &mut Vec<MaterialAlias>),
}

impl Clone for Entry {
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            value: (self.copy)(self.value.as_ref()),
            copy: self.copy,
            turn: self.turn,
            rig: self.rig,
            describe: self.describe,
            alias: self.alias,
        }
    }
}

fn copy_of<E: Extension>(value: &(dyn Any + Send + Sync)) -> BoxedValue {
    Box::new(downcast::<E>(value).clone())
}

fn convert_as<E: Extension>(value: &mut (dyn Any + Send + Sync), conversion: &Conversion) {
    value
        .downcast_mut::<E>()
        .expect("extensions are stored under their own type")
        .convert(conversion);
}

fn rig_of<E: Extension>(value: &(dyn Any + Send + Sync), model: &Model) -> Option<Box<dyn Rig>> {
    downcast::<E>(value).rig(model)
}

fn describe_as<E: Extension>(value: &(dyn Any + Send + Sync), facts: &mut RigFacts) {
    downcast::<E>(value).describe_rig(facts);
}

fn alias_as<E: Extension>(value: &(dyn Any + Send + Sync), aliases: &mut Vec<MaterialAlias>) {
    downcast::<E>(value).alias_materials(aliases);
}

fn downcast<E: Extension>(value: &(dyn Any + Send + Sync)) -> &E {
    value
        .downcast_ref::<E>()
        .expect("extensions are stored under their own type")
}

#[derive(Clone, Default)]
pub struct Extensions {
    entries: FxHashMap<TypeId, Entry>,
}

impl Extensions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<E: Extension>(&mut self, extension: E) {
        self.entries.insert(
            TypeId::of::<E>(),
            Entry {
                name: E::NAME,
                value: Box::new(extension),
                copy: copy_of::<E>,
                turn: convert_as::<E>,
                rig: rig_of::<E>,
                describe: describe_as::<E>,
                alias: alias_as::<E>,
            },
        );
    }

    pub fn convert(&mut self, conversion: &Conversion) {
        for entry in self.entries.values_mut() {
            (entry.turn)(entry.value.as_mut(), conversion);
        }
    }

    #[must_use]
    pub fn rig(&self, model: &Model) -> Option<Box<dyn Rig>> {
        self.ordered()
            .into_iter()
            .find_map(|entry| (entry.rig)(entry.value.as_ref(), model))
    }

    pub(crate) fn describe_rig(&self, facts: &mut RigFacts) {
        for entry in self.ordered() {
            (entry.describe)(entry.value.as_ref(), facts);
        }
    }

    pub(crate) fn alias_materials(&self, aliases: &mut Vec<MaterialAlias>) {
        for entry in self.ordered() {
            (entry.alias)(entry.value.as_ref(), aliases);
        }
    }

    fn ordered(&self) -> Vec<&Entry> {
        let mut entries: Vec<&Entry> = self.entries.values().collect();
        entries.sort_by_key(|entry| entry.name);
        entries
    }

    #[must_use]
    pub fn get<E: Extension>(&self) -> Option<&E> {
        self.entries
            .get(&TypeId::of::<E>())
            .and_then(|entry| entry.value.downcast_ref())
    }

    pub fn get_mut<E: Extension>(&mut self) -> Option<&mut E> {
        self.entries
            .get_mut(&TypeId::of::<E>())
            .and_then(|entry| entry.value.downcast_mut())
    }

    pub fn remove<E: Extension>(&mut self) -> Option<E> {
        self.entries
            .remove(&TypeId::of::<E>())
            .and_then(|entry| entry.value.downcast().ok())
            .map(|value| *value)
    }

    #[must_use]
    pub fn contains<E: Extension>(&self) -> bool {
        self.entries.contains_key(&TypeId::of::<E>())
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = &'static str> {
        self.entries.values().map(|entry| entry.name)
    }
}

impl std::fmt::Debug for Extensions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.names()).finish()
    }
}
