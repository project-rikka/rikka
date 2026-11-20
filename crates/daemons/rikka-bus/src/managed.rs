use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use zbus::Message;
use zbus::blocking::Connection;
use zbus::message::Sequence;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use crate::properties::Properties;
use crate::signals::{Coalesce, Rule, Signals, Watch};

const OBJECT_MANAGER: &str = "org.freedesktop.DBus.ObjectManager";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";
const BUS: &str = "org.freedesktop.DBus";
const INTERFACES_ADDED: &str = "InterfacesAdded";
const INTERFACES_REMOVED: &str = "InterfacesRemoved";
const PROPERTIES_CHANGED: &str = "PropertiesChanged";
const NAME_OWNER_CHANGED: &str = "NameOwnerChanged";

const SETTLE: Duration = Duration::from_millis(50);
const SETTLE_AT_MOST: Duration = Duration::from_millis(250);

type Interfaces = HashMap<String, HashMap<String, OwnedValue>>;

#[derive(Debug)]
pub struct ManagedObjects {
    service: String,
    root: String,
    objects: BTreeMap<String, HashMap<String, Entry>>,
    read_at: Sequence,
}

#[derive(Debug)]
struct Entry {
    properties: Properties,
    since: Sequence,
}

#[derive(Debug, Default)]
pub struct ObjectChanges(Vec<Change>);

#[derive(Debug)]
struct Change {
    at: Sequence,
    kind: ChangeKind,
}

#[derive(Debug)]
enum ChangeKind {
    Added {
        path: String,
        interfaces: Interfaces,
    },
    Removed {
        path: String,
        interfaces: Vec<String>,
    },
    Changed {
        path: String,
        interface: String,
        changed: HashMap<String, OwnedValue>,
        invalidated: Vec<String>,
    },
    Owner {
        present: bool,
    },
}

impl Coalesce for ObjectChanges {
    fn merge(&mut self, other: ObjectChanges) {
        self.0.extend(other.0);
    }
}

impl ObjectChanges {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn parse(message: &Message) -> Option<ObjectChanges> {
        let header = message.header();
        let member = header.member()?.as_str();
        let body = message.body();
        let kind = match member {
            INTERFACES_ADDED => {
                let (path, interfaces) =
                    body.deserialize::<(OwnedObjectPath, Interfaces)>().ok()?;
                ChangeKind::Added {
                    path: path.as_str().to_owned(),
                    interfaces,
                }
            }
            INTERFACES_REMOVED => {
                let (path, interfaces) =
                    body.deserialize::<(OwnedObjectPath, Vec<String>)>().ok()?;
                ChangeKind::Removed {
                    path: path.as_str().to_owned(),
                    interfaces,
                }
            }
            PROPERTIES_CHANGED => {
                let (interface, changed, invalidated) = body
                    .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
                    .ok()?;
                ChangeKind::Changed {
                    path: header.path()?.as_str().to_owned(),
                    interface,
                    changed,
                    invalidated,
                }
            }
            NAME_OWNER_CHANGED => {
                let (_, _, owner) = body.deserialize::<(&str, &str, &str)>().ok()?;
                ChangeKind::Owner {
                    present: !owner.is_empty(),
                }
            }
            _ => return None,
        };
        Some(ObjectChanges(vec![Change {
            at: message.recv_position(),
            kind,
        }]))
    }
}

impl ManagedObjects {
    pub fn read(
        connection: &Connection,
        service: &str,
        root: &str,
    ) -> zbus::Result<ManagedObjects> {
        let reply = connection.call_method(
            Some(service),
            root,
            Some(OBJECT_MANAGER),
            "GetManagedObjects",
            &(),
        )?;
        let read_at = reply.recv_position();
        let objects: HashMap<OwnedObjectPath, Interfaces> = reply.body().deserialize()?;
        Ok(ManagedObjects {
            service: service.to_owned(),
            root: root.to_owned(),
            objects: objects
                .into_iter()
                .map(|(path, interfaces)| (path.as_str().to_owned(), entries(interfaces, read_at)))
                .collect(),
            read_at,
        })
    }

    pub fn watch(
        connection: &Connection,
        service: &str,
        root: &str,
        deliver: impl Fn(ObjectChanges) -> bool + Send + 'static,
    ) -> zbus::Result<Watch> {
        Signals::new()
            .rule(
                Rule::new()
                    .sender(service)
                    .interface(OBJECT_MANAGER)
                    .path(root),
            )
            .rule(
                Rule::new()
                    .sender(service)
                    .interface(PROPERTIES)
                    .member(PROPERTIES_CHANGED)
                    .path_namespace(root),
            )
            .rule(
                Rule::new()
                    .sender(BUS)
                    .interface(BUS)
                    .member(NAME_OWNER_CHANGED)
                    .arg(0, service),
            )
            .debounce(SETTLE, SETTLE_AT_MOST)
            .listen(connection, ObjectChanges::parse, deliver)
    }

    pub fn follow(
        &mut self,
        connection: &Connection,
        changes: ObjectChanges,
    ) -> zbus::Result<bool> {
        let mut moved = false;
        for Change { at, kind } in changes.0 {
            if at < self.read_at {
                continue;
            }
            moved |= match kind {
                ChangeKind::Owner { present: true } => {
                    *self = ManagedObjects::read(connection, &self.service, &self.root)?;
                    true
                }
                ChangeKind::Owner { present: false } => {
                    self.read_at = at;
                    let emptied = !self.objects.is_empty();
                    self.objects.clear();
                    emptied
                }
                ChangeKind::Added { path, interfaces } => {
                    self.objects
                        .entry(path)
                        .or_default()
                        .extend(entries(interfaces, at));
                    true
                }
                ChangeKind::Removed { path, interfaces } => self.remove(&path, &interfaces, at),
                ChangeKind::Changed {
                    path,
                    interface,
                    changed,
                    invalidated,
                } => self.change(connection, &path, &interface, changed, &invalidated, at)?,
            };
        }
        Ok(moved)
    }

    #[must_use]
    pub fn get(&self, path: &str, interface: &str) -> Option<&Properties> {
        Some(&self.objects.get(path)?.get(interface)?.properties)
    }

    pub fn implementing<'a>(
        &'a self,
        interface: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a Properties)> {
        self.objects.iter().filter_map(move |(path, interfaces)| {
            Some((path.as_str(), &interfaces.get(interface)?.properties))
        })
    }

    fn remove(&mut self, path: &str, interfaces: &[String], at: Sequence) -> bool {
        let Some(object) = self.objects.get_mut(path) else {
            return false;
        };
        let before = object.len();
        object.retain(|name, entry| entry.since > at || !interfaces.contains(name));
        let moved = object.len() != before;
        if object.is_empty() {
            self.objects.remove(path);
        }
        moved
    }

    fn change(
        &mut self,
        connection: &Connection,
        path: &str,
        interface: &str,
        changed: HashMap<String, OwnedValue>,
        invalidated: &[String],
        at: Sequence,
    ) -> zbus::Result<bool> {
        let Some(entry) = self
            .objects
            .get_mut(path)
            .and_then(|object| object.get_mut(interface))
        else {
            return Ok(false);
        };
        if at < entry.since {
            return Ok(false);
        }
        entry.properties.merge(Properties::from(changed));
        if !invalidated.is_empty() {
            let (properties, since) =
                Properties::read_at(connection, &self.service, path, interface)?;
            *entry = Entry { properties, since };
        }
        Ok(true)
    }
}

fn entries(interfaces: Interfaces, since: Sequence) -> HashMap<String, Entry> {
    interfaces
        .into_iter()
        .map(|(name, properties)| {
            (
                name,
                Entry {
                    properties: Properties::from(properties),
                    since,
                },
            )
        })
        .collect()
}
