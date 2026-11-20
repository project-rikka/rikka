use std::io;

use crate::authenticator::Authenticator;
use crate::ceremony::{Event, Watch};
use crate::error::{Error, Result};
use crate::hid::Transport;

pub(crate) fn connect_all(transport: &dyn Transport, watch: &Watch) -> Result<Vec<Authenticator>> {
    let mut hotplug = transport.hotplug().map_err(Error::io)?;
    let mut announced = false;

    loop {
        watch.check()?;

        let mut connected = Vec::new();
        let mut denied = None;
        for device in transport.devices().map_err(Error::io)? {
            match transport.open(&device) {
                Ok(link) => match Authenticator::connect(device.clone(), link, watch) {
                    Ok(authenticator) => connected.push(authenticator),
                    Err(error) if error.is_stop() => return Err(error),
                    Err(error) => {
                        tracing::warn!(device = device.path(), %error, "security key not responding");
                    }
                },
                Err(error) if error.kind() == io::ErrorKind::PermissionDenied => {
                    denied = Some(device.path().to_owned());
                }
                Err(error) => {
                    tracing::debug!(device = device.path(), %error, "security key disconnected");
                }
            }
        }

        if !connected.is_empty() {
            return Ok(connected);
        }
        if let Some(path) = denied {
            return Err(Error::AccessDenied(path));
        }
        if !announced {
            watch.emit(Event::Insert);
            announced = true;
        }
        watch.wait_for_hotplug(hotplug.as_mut())?;
    }
}
