use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::zvariant::ObjectPath;

use crate::connect;
use crate::error::{Error, Result};
use crate::event::{Event, Secrets};
use crate::profile::{self, Profile};
use crate::request::{Join, JoinTarget, Request};
use crate::session::proxies::{
    AccessPointObjectProxy, DeviceProxy, ManagerProxyBlocking, SavedConnectionProxyBlocking,
    SettingsStoreProxyBlocking, WirelessDeviceProxy,
};
use crate::ssid::Ssid;
use crate::values;
use rikka_bus::DaemonError;

const ANYWHERE: &str = "/";

pub(crate) fn perform(
    connection: &Connection,
    request: Request,
    user: &str,
) -> Result<Option<Event>> {
    match request {
        Request::Scan(device) => {
            let radio = WirelessDeviceProxy::builder(connection)
                .path(device.as_str().to_owned())?
                .build()?;
            match radio.request_scan(HashMap::new()) {
                Err(zbus::Error::MethodError(name, _, _))
                    if name.as_str() == "org.freedesktop.NetworkManager.Device.NotAllowed" =>
                {
                    Ok(None)
                }
                Err(error) => Err(error.into()),
                Ok(()) => Ok(None),
            }
        }
        Request::Radio(on) => {
            manager(connection)?.set_wireless_enabled(on)?;
            Ok(None)
        }
        Request::Networking(on) => {
            manager(connection)?.enable(on)?;
            Ok(None)
        }
        Request::Join(join) => {
            join_network(connection, &join, user)?;
            Ok(None)
        }
        Request::Activate { uuid, device } => {
            let saved = by_uuid(connection, &uuid)?;
            let device = device.map_or_else(|| ANYWHERE.to_owned(), |device| device.0);
            manager(connection)?.activate_connection(
                &path(&saved)?,
                &path(&device)?,
                &path(ANYWHERE)?,
            )?;
            Ok(None)
        }
        Request::Deactivate(active) => {
            manager(connection)?.deactivate_connection(&path(active.as_str())?)?;
            Ok(None)
        }
        Request::Disconnect(device) => {
            DeviceProxy::builder(connection)
                .path(device.as_str().to_owned())?
                .build()?
                .disconnect()?;
            Ok(None)
        }
        Request::Forget(uuid) => {
            saved(connection, &uuid)?.delete()?;
            Ok(None)
        }
        Request::Save(profile) => {
            save(connection, &profile, user)?;
            Ok(None)
        }
        Request::Reveal(uuid) => {
            let secrets = reveal(connection, &uuid)?;
            Ok(Some(Event::Revealed { uuid, secrets }))
        }
        Request::Hotspot(hotspot) => {
            let draft = hotspot.draft(user)?;
            manager(connection)?.add_and_activate_connection(
                &draft,
                &path(hotspot.device.as_str())?,
                &path(ANYWHERE)?,
            )?;
            Ok(None)
        }
        Request::Import(file) => {
            let draft = crate::tunnel::WireGuardTunnel::from_file(&file)?.draft(user)?;
            store(connection)?.add_connection(&draft)?;
            Ok(None)
        }
        Request::Recheck => {
            manager(connection)?.check_connectivity()?;
            Ok(None)
        }
    }
}

fn join_network(connection: &Connection, join: &Join, user: &str) -> Result<()> {
    let (point, name) = match &join.network {
        JoinTarget::Visible(point) => (
            point.as_str().to_owned(),
            ssid_of(connection, point.as_str())?,
        ),
        JoinTarget::Hidden(name) => (ANYWHERE.to_owned(), name.clone()),
    };
    let draft = connect::draft(join, &name, user)?;
    manager(connection)?.add_and_activate_connection(
        &draft,
        &path(join.device.as_str())?,
        &path(&point)?,
    )?;
    Ok(())
}

fn save(connection: &Connection, profile: &Profile, user: &str) -> Result<()> {
    let saved = saved(connection, &profile.uuid)?;
    let mut draft = values::draft(&values::settings(saved.get_settings()?));
    profile::merge(&mut draft, profile, user);
    saved.update(&draft)?;
    Ok(())
}

fn reveal(connection: &Connection, uuid: &str) -> Result<Secrets> {
    let saved = saved(connection, uuid)?;
    let wireless = saved
        .get_secrets(profile::dbus::SECURITY)
        .ok()
        .map(values::settings);
    let account = saved
        .get_secrets(profile::dbus::EAP)
        .ok()
        .map(values::settings);
    if wireless.is_none() && account.is_none() {
        return Err(Error::unauthorized("the saved password could not be read"));
    }
    let eap = account
        .as_ref()
        .and_then(|settings| settings.get(profile::dbus::EAP));
    Ok(Secrets {
        passphrase: wireless
            .as_ref()
            .and_then(|settings| settings.get(profile::dbus::SECURITY))
            .and_then(|security| {
                security
                    .string("psk")
                    .or_else(|| security.string("wep-key0"))
            }),
        password: eap.and_then(|eap| eap.string("password")),
        private_key_password: eap.and_then(|eap| eap.string("private-key-password")),
    })
}

fn manager(connection: &Connection) -> Result<ManagerProxyBlocking<'static>> {
    Ok(ManagerProxyBlocking::new(connection)?)
}

fn store(connection: &Connection) -> Result<SettingsStoreProxyBlocking<'static>> {
    Ok(SettingsStoreProxyBlocking::new(connection)?)
}

fn by_uuid(connection: &Connection, uuid: &str) -> Result<String> {
    Ok(store(connection)?
        .get_connection_by_uuid(uuid)
        .map_err(|_| Error::gone())?
        .as_str()
        .to_owned())
}

fn saved(connection: &Connection, uuid: &str) -> Result<SavedConnectionProxyBlocking<'static>> {
    Ok(SavedConnectionProxyBlocking::builder(connection)
        .path(by_uuid(connection, uuid)?)?
        .build()?)
}

fn path(path: &str) -> Result<ObjectPath<'static>> {
    Ok(ObjectPath::try_from(path.to_owned())?)
}

fn ssid_of(connection: &Connection, point: &str) -> Result<Ssid> {
    let point = AccessPointObjectProxy::builder(connection)
        .path(point.to_owned())?
        .build()?;
    Ok(Ssid::new(point.ssid().map_err(|_| Error::gone())?))
}
