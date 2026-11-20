use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use pipewire::registry::GlobalObject;
use pipewire::spa::utils::dict::DictRef;
use pipewire::types::ObjectType;
use rikka_pipewire::{Connection, LoopWorker};

use crate::device_watch::DeviceEvent;
use crate::error::Result;
use crate::spawn;

use super::graph::Device;

const THREAD: &str = "rikka-audio-devices";

#[derive(Debug)]
pub(crate) struct Watcher {
    _worker: LoopWorker,
}

impl Watcher {
    pub(crate) fn start(on_event: impl FnMut(DeviceEvent) + Send + 'static) -> Result<Watcher> {
        let worker = rikka_pipewire::spawn(THREAD, move |stopping| {
            let connection = Connection::open()?;
            Ok(move || {
                let tracker = Rc::new(RefCell::new(Tracker {
                    devices: HashSet::new(),
                    on_event,
                }));
                let added = Rc::clone(&tracker);
                let removed = Rc::clone(&tracker);
                let _listener = connection
                    .registry()
                    .add_listener_local()
                    .global(move |global| added.borrow_mut().add(global))
                    .global_remove(move |id| removed.borrow_mut().remove(id))
                    .register();
                connection.run_until_stopped(stopping);
            })
        })
        .map_err(spawn::failed(THREAD))?;
        Ok(Watcher { _worker: worker })
    }
}

struct Tracker<F> {
    devices: HashSet<u32>,
    on_event: F,
}

impl<F: FnMut(DeviceEvent)> Tracker<F> {
    fn add(&mut self, global: &GlobalObject<&DictRef>) {
        if global.type_ != ObjectType::Node {
            return;
        }
        let Some(device) = global.props.and_then(Device::read) else {
            return;
        };
        if self.devices.insert(global.id) {
            (self.on_event)(DeviceEvent::Added {
                id: global.id,
                name: device.label,
                direction: device.direction,
            });
        }
    }

    fn remove(&mut self, id: u32) {
        if self.devices.remove(&id) {
            (self.on_event)(DeviceEvent::Removed { id });
        }
    }
}
