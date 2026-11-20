use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::zvariant::{ObjectPath, Value};

use crate::error::Result;
use crate::request::Request;
use crate::session::proxies::{AdapterProxy, DeviceProxy};

pub(crate) fn perform(connection: &Connection, request: Request) -> Result<()> {
    match request {
        Request::Power(id, on) => adapter(connection, id.as_str())?.set_powered(on)?,
        Request::Discoverable(id, on) => {
            adapter(connection, id.as_str())?.set_discoverable(on)?;
        }
        Request::Pairable(id, on) => adapter(connection, id.as_str())?.set_pairable(on)?,
        Request::Rename(id, name) => adapter(connection, id.as_str())?.set_alias(&name)?,
        Request::Scan(id, on) => {
            let adapter = adapter(connection, id.as_str())?;
            if !on {
                adapter.stop_discovery()?;
                return Ok(());
            }
            if let Err(error) = adapter.set_discovery_filter(HashMap::from([
                ("Transport", Value::from("auto")),
                ("DuplicateData", Value::from(true)),
            ])) {
                tracing::debug!(%error, "SetDiscoveryFilter failed, discovering unfiltered");
            }
            adapter.start_discovery()?;
        }
        Request::Pair(id) => device(connection, id.as_str())?.pair()?,
        Request::Cancel(id) => device(connection, id.as_str())?.cancel_pairing()?,
        Request::Connect(id) => device(connection, id.as_str())?.connect()?,
        Request::Disconnect(id) => device(connection, id.as_str())?.disconnect()?,
        Request::Trust(id, on) => device(connection, id.as_str())?.set_trusted(on)?,
        Request::Block(id, on) => device(connection, id.as_str())?.set_blocked(on)?,
        Request::Label(id, name) => device(connection, id.as_str())?.set_alias(&name)?,
        Request::Forget(id) => forget(connection, id.as_str())?,
    }
    Ok(())
}

fn forget(connection: &Connection, path: &str) -> Result<()> {
    let device = device(connection, path)?;
    let adapter = device.adapter()?;
    self::adapter(connection, adapter.as_str())?.remove_device(&object(path)?)?;
    Ok(())
}

fn adapter(connection: &Connection, path: &str) -> Result<AdapterProxy<'static>> {
    Ok(AdapterProxy::builder(connection)
        .path(path.to_owned())?
        .build()?)
}

fn device(connection: &Connection, path: &str) -> Result<DeviceProxy<'static>> {
    Ok(DeviceProxy::builder(connection)
        .path(path.to_owned())?
        .build()?)
}

fn object(path: &str) -> Result<ObjectPath<'static>> {
    Ok(ObjectPath::try_from(path.to_owned())?)
}
