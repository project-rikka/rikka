use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::message::Sequence;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

use crate::reading::{self, NULL_OBJECT_PATH};

const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

#[derive(Debug, Default)]
pub struct Properties(HashMap<String, OwnedValue>);

impl Properties {
    pub fn read(
        connection: &Connection,
        service: &str,
        path: &str,
        interface: &str,
    ) -> zbus::Result<Properties> {
        Properties::read_at(connection, service, path, interface).map(|(properties, _)| properties)
    }

    pub(crate) fn read_at(
        connection: &Connection,
        service: &str,
        path: &str,
        interface: &str,
    ) -> zbus::Result<(Properties, Sequence)> {
        let reply = connection.call_method(
            Some(service),
            path,
            Some(PROPERTIES),
            "GetAll",
            &(interface,),
        )?;
        let properties: HashMap<String, OwnedValue> = reply.body().deserialize()?;
        Ok((Properties(properties), reply.recv_position()))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &OwnedValue)> {
        self.0.iter().map(|(key, value)| (key.as_str(), value))
    }

    #[must_use]
    pub fn value<T: TryFrom<OwnedValue>>(&self, key: &str) -> Option<T> {
        T::try_from(self.0.get(key)?.try_clone().ok()?).ok()
    }

    #[must_use]
    pub fn list<T>(&self, key: &str) -> Option<Vec<T>>
    where
        Vec<T>: TryFrom<OwnedValue>,
    {
        self.value::<Vec<T>>(key)
    }

    #[must_use]
    pub fn maps(&self, key: &str) -> Option<Vec<Properties>> {
        self.list::<HashMap<String, OwnedValue>>(key)
            .map(|maps| maps.into_iter().map(Properties).collect())
    }

    #[must_use]
    pub fn string_ref(&self, key: &str) -> Option<&str> {
        reading::text(self.0.get(key)?)
    }

    #[must_use]
    pub fn string(&self, key: &str) -> Option<String> {
        self.string_ref(key).map(str::to_owned)
    }

    #[must_use]
    pub fn strings(&self, key: &str) -> Option<Vec<String>> {
        self.0.get(key).map(|value| reading::texts(value))
    }

    #[must_use]
    pub fn boolean(&self, key: &str) -> Option<bool> {
        reading::boolean(self.0.get(key)?)
    }

    #[must_use]
    pub fn int64(&self, key: &str) -> Option<i64> {
        reading::int64(self.0.get(key)?)
    }

    #[must_use]
    pub fn uint32(&self, key: &str) -> Option<u32> {
        u32::try_from(self.int64(key)?).ok()
    }

    #[must_use]
    pub fn uint64(&self, key: &str) -> Option<u64> {
        reading::uint64(self.0.get(key)?)
    }

    #[must_use]
    pub fn double(&self, key: &str) -> Option<f64> {
        reading::double(self.0.get(key)?)
    }

    #[must_use]
    pub fn object_path(&self, key: &str) -> Option<&str> {
        reading::object_path(self.0.get(key)?)
    }

    #[must_use]
    pub fn object_paths(&self, key: &str) -> Option<Vec<String>> {
        self.list::<OwnedObjectPath>(key).map(|paths| {
            paths
                .into_iter()
                .map(|path| path.as_str().to_owned())
                .filter(|path| path != NULL_OBJECT_PATH)
                .collect()
        })
    }

    pub(crate) fn merge(&mut self, changed: Properties) {
        self.0.extend(changed.0);
    }
}

impl From<HashMap<String, OwnedValue>> for Properties {
    fn from(properties: HashMap<String, OwnedValue>) -> Properties {
        Properties(properties)
    }
}
