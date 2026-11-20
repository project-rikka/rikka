use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rikka_bus::ManagedObjects;
use zbus::blocking::Connection;
use zbus::zvariant::Value;

use crate::error::Error;
use crate::following::{Cache, Following};
use crate::objects;
use crate::volume::{Volume, VolumeId};

pub(crate) const SERVICE: &str = "org.freedesktop.UDisks2";
pub(crate) const ROOT: &str = "/org/freedesktop/UDisks2";

#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.Filesystem",
    default_service = "org.freedesktop.UDisks2"
)]
trait Filesystem {
    fn mount(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<String>;
    fn unmount(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.Drive",
    default_service = "org.freedesktop.UDisks2"
)]
trait Drive {
    fn eject(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
    fn power_off(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.UDisks2.Encrypted",
    default_service = "org.freedesktop.UDisks2"
)]
trait Encrypted {
    fn unlock(
        &self,
        passphrase: &str,
        options: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
    fn lock(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;
}

#[derive(Debug)]
pub struct Drives {
    connection: Option<Connection>,
    cache: Cache,
    _following: Option<Following>,
}

impl Drives {
    #[must_use]
    pub fn open(changed: impl Fn() + Send + 'static) -> Self {
        let connection = Connection::system()
            .inspect_err(|error| tracing::warn!(%error, "cannot reach the system bus"))
            .ok();
        let cache = Cache::default();
        let following = connection
            .as_ref()
            .and_then(|connection| Following::start(connection, Arc::clone(&cache), changed));
        Self {
            connection,
            cache,
            _following: following,
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> Vec<Volume> {
        self.objects(objects::volumes)
            .unwrap_or_else(|_| crate::fallback::mounted())
    }

    pub fn mount(&self, id: &VolumeId, interactive: bool) -> Result<PathBuf, Error> {
        let filesystem = FilesystemProxyBlocking::builder(self.connection()?)
            .path(id.as_str())?
            .build()?;
        match filesystem.mount(authorization(interactive)) {
            Ok(at) => Ok(PathBuf::from(at)),
            Err(refusal) => match Error::from(refusal) {
                Error::AlreadyMounted => self
                    .objects(|objects| objects::mounted_at(objects, id.as_str()))?
                    .ok_or(Error::AlreadyMounted),
                other => Err(other),
            },
        }
    }

    pub fn unmount(&self, id: &VolumeId) -> Result<(), Error> {
        let filesystem = FilesystemProxyBlocking::builder(self.connection()?)
            .path(id.as_str())?
            .build()?;
        match filesystem.unmount(authorization(true)) {
            Ok(()) => Ok(()),
            Err(refusal) => match Error::from(refusal) {
                Error::NotMounted => Ok(()),
                other => Err(other),
            },
        }
    }

    pub fn eject(&self, id: &VolumeId) -> Result<(), Error> {
        let (mounted, drive) = self.objects(|objects| {
            let drive = objects::drive_path(objects, id.as_str());
            let ejectable = drive
                .as_deref()
                .is_some_and(|drive| objects::is_ejectable(objects, drive));
            (
                objects::mounted_at(objects, id.as_str()).is_some(),
                drive.map(|drive| (drive, ejectable)),
            )
        })?;
        if mounted {
            self.unmount(id)?;
        }
        let Some((drive, ejectable)) = drive else {
            return Err(Error::NotSupported);
        };
        let proxy = DriveProxyBlocking::builder(self.connection()?)
            .path(drive.as_str())?
            .build()?;
        if ejectable {
            proxy.eject(authorization(true))?;
            return Ok(());
        }
        proxy.power_off(authorization(true))?;
        Ok(())
    }

    pub fn unlock(&self, id: &VolumeId, passphrase: &str) -> Result<VolumeId, Error> {
        let encrypted = EncryptedProxyBlocking::builder(self.connection()?)
            .path(id.as_str())?
            .build()?;
        let unlocked = encrypted.unlock(passphrase, authorization(true))?;
        Ok(VolumeId(unlocked.as_str().to_owned()))
    }

    pub fn lock(&self, id: &VolumeId) -> Result<(), Error> {
        let Some(backing) = self.objects(|objects| objects::backing_path(objects, id.as_str()))?
        else {
            return Err(Error::NotSupported);
        };
        let encrypted = EncryptedProxyBlocking::builder(self.connection()?)
            .path(backing.as_str())?
            .build()?;
        encrypted.lock(authorization(true))?;
        Ok(())
    }

    fn connection(&self) -> Result<&Connection, Error> {
        self.connection.as_ref().ok_or(Error::Unavailable)
    }

    fn objects<T>(&self, read: impl FnOnce(&ManagedObjects) -> T) -> Result<T, Error> {
        let mut cache = self.cache.lock();
        let objects = match cache.take() {
            Some(objects) => objects,
            None => ManagedObjects::read(self.connection()?, SERVICE, ROOT)?,
        };
        Ok(read(cache.insert(objects)))
    }
}

fn authorization(interactive: bool) -> HashMap<&'static str, Value<'static>> {
    if interactive {
        return HashMap::new();
    }
    HashMap::from([("auth.no_user_interaction", Value::Bool(true))])
}
