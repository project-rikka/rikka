use std::collections::{HashMap, HashSet};

use crate::error::{Error, Result};
use crate::model::{Arg, ArgKind, Enum, Interface, Protocol};
use crate::names;

const GENERATED_NAMES: &[&str] = &["Request", "Event"];

pub(crate) struct Context<'p> {
    interfaces: HashMap<&'p str, &'p Interface>,
    bindable: HashSet<&'p str>,
    runtime: String,
}

pub(crate) struct EnumType<'p> {
    pub(crate) path: String,
    pub(crate) definition: &'p Enum,
}

impl<'p> Context<'p> {
    pub(crate) fn new(protocols: &'p [Protocol], runtime: &str) -> Result<Self> {
        let mut interfaces = HashMap::new();
        for interface in protocols.iter().flat_map(|protocol| &protocol.interfaces) {
            if interfaces
                .insert(interface.name.as_str(), interface)
                .is_some()
            {
                return Err(Error::Inconsistent(format!(
                    "interface {} is declared twice",
                    interface.name
                )));
            }
        }
        let created: HashSet<&str> = interfaces
            .values()
            .flat_map(|interface| interface.requests.iter().chain(&interface.events))
            .flat_map(|message| &message.args)
            .filter(|arg| arg.kind == ArgKind::NewId)
            .filter_map(|arg| arg.interface.as_deref())
            .collect();
        let bindable = interfaces
            .values()
            .filter(|interface| {
                interface.name != "wl_display"
                    && !interface.frozen
                    && !created.contains(interface.name.as_str())
            })
            .map(|interface| interface.name.as_str())
            .collect();
        let context = Self {
            interfaces,
            bindable,
            runtime: runtime.to_owned(),
        };
        for interface in context.interfaces.values() {
            context.validate(interface)?;
        }
        Ok(context)
    }

    pub(crate) fn runtime(&self) -> &str {
        &self.runtime
    }

    pub(crate) fn bindable(&self, interface: &Interface) -> bool {
        self.bindable.contains(interface.name.as_str())
    }

    pub(crate) fn metadata(&self, from: &Interface, target: &str) -> String {
        if from.name == target {
            "INTERFACE".to_owned()
        } else {
            format!("super::{target}::INTERFACE")
        }
    }

    pub(crate) fn handle(&self, from: &Interface, target: &str) -> String {
        if from.name == target {
            names::camel(target)
        } else {
            format!("super::{target}::{}", names::camel(target))
        }
    }

    pub(crate) fn enum_type(&self, from: &'p Interface, reference: &str) -> Result<EnumType<'p>> {
        let (owner, name) = match reference.split_once('.') {
            Some((owner, name)) => (
                *self.interfaces.get(owner).ok_or_else(|| {
                    Error::Inconsistent(format!(
                        "{} refers to enum {reference} of an unknown interface",
                        from.name
                    ))
                })?,
                name,
            ),
            None => (from, reference),
        };
        let definition = owner
            .enums
            .iter()
            .find(|candidate| candidate.name == name)
            .ok_or_else(|| {
                Error::Inconsistent(format!(
                    "{} refers to enum {reference}, which does not exist",
                    from.name
                ))
            })?;
        let path = if owner.name == from.name {
            names::camel(name)
        } else {
            format!("super::{}::{}", owner.name, names::camel(name))
        };
        Ok(EnumType { path, definition })
    }

    fn validate(&self, interface: &'p Interface) -> Result<()> {
        let handle = names::camel(&interface.name);
        for definition in &interface.enums {
            let name = names::camel(&definition.name);
            if GENERATED_NAMES.contains(&name.as_str()) || name == handle {
                return Err(Error::Inconsistent(format!(
                    "enum {}.{} would clash with a generated item",
                    interface.name, definition.name
                )));
            }
        }
        for message in interface.requests.iter().chain(&interface.events) {
            for arg in &message.args {
                self.validate_arg(interface, &message.name, arg)?;
            }
        }
        Ok(())
    }

    fn validate_arg(&self, interface: &'p Interface, message: &str, arg: &Arg) -> Result<()> {
        if let Some(target) = &arg.interface
            && !self.interfaces.contains_key(target.as_str())
        {
            return Err(Error::Inconsistent(format!(
                "{}.{message} argument {} refers to unknown interface {target}",
                interface.name, arg.name
            )));
        }
        if let Some(reference) = &arg.enumeration {
            self.enum_type(interface, reference)?;
        }
        Ok(())
    }
}
