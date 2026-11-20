use crate::backend::Watcher;
use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeviceDirection {
    Input,
    Output,
    Duplex,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceEvent {
    Added {
        id: u32,
        name: String,
        direction: DeviceDirection,
    },
    Removed {
        id: u32,
    },
}

#[derive(Debug)]
pub struct DeviceWatch {
    _watcher: Watcher,
}

impl DeviceWatch {
    pub fn start(on_event: impl FnMut(DeviceEvent) + Send + 'static) -> Result<DeviceWatch> {
        Ok(DeviceWatch {
            _watcher: Watcher::start(on_event)?,
        })
    }
}
