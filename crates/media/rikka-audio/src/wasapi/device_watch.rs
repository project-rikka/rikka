use std::collections::{HashMap, HashSet};
use std::os::windows::io::AsHandle;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rikka_worker::{EventFd, Worker, event_fd};
use windows::Win32::Media::Audio::{EDataFlow, eCapture, eRender};

use crate::device_watch::{DeviceDirection, DeviceEvent};
use crate::error::Result;
use crate::spawn;

use super::com::Apartment;
use super::endpoint::Endpoints;
use super::host::Stopper;
use super::notify;

const THREAD: &str = "rikka-audio-devices";
const FLOWS: [(EDataFlow, DeviceDirection); 2] = [
    (eRender, DeviceDirection::Output),
    (eCapture, DeviceDirection::Input),
];

#[derive(Debug)]
pub(crate) struct Watcher {
    _worker: Worker<Stopper>,
}

impl Watcher {
    pub(crate) fn start(on_event: impl FnMut(DeviceEvent) + Send + 'static) -> Result<Watcher> {
        let (stopper, stopping, stopped) = Stopper::new()?;
        let worker = Worker::spawn(THREAD, stopper, move || {
            let apartment = Apartment::enter()?;
            let endpoints = Endpoints::new()?;
            let (changed_bell, changed) = event_fd()?;
            let registration = endpoints.register(notify::listener(changed_bell))?;
            Ok(move || {
                let mut tracker = Tracker {
                    ids: HashMap::new(),
                    present: HashSet::new(),
                    on_event,
                };
                tracker.watch(&endpoints, &stopping, &stopped, &changed);
                drop(registration);
                drop(apartment);
            })
        })
        .map_err(spawn::failed(THREAD))?;
        Ok(Watcher { _worker: worker })
    }
}

struct Tracker<F> {
    ids: HashMap<String, u32>,
    present: HashSet<u32>,
    on_event: F,
}

impl<F: FnMut(DeviceEvent)> Tracker<F> {
    fn watch(
        &mut self,
        endpoints: &Endpoints,
        stopping: &Arc<AtomicBool>,
        stopped: &EventFd,
        changed: &EventFd,
    ) {
        loop {
            changed.drain();
            self.scan(endpoints);
            if let Err(error) = stopped.wait_signaled([changed.as_handle()], None) {
                tracing::warn!(%error, "audio device watch cannot wait");
                return;
            }
            if stopping.load(Ordering::Acquire) {
                return;
            }
        }
    }

    fn scan(&mut self, endpoints: &Endpoints) {
        let mut seen = HashSet::new();
        for (flow, direction) in FLOWS {
            let listed = match endpoints.list(flow) {
                Ok(listed) => listed,
                Err(error) => {
                    tracing::warn!(%error, "failed to list audio devices");
                    return;
                }
            };
            for endpoint in listed {
                let next = self.ids.len() as u32;
                let id = *self.ids.entry(endpoint.id).or_insert(next);
                seen.insert(id);
                if self.present.insert(id) {
                    (self.on_event)(DeviceEvent::Added {
                        id,
                        name: endpoint.name,
                        direction,
                    });
                }
            }
        }
        let gone: Vec<u32> = self.present.difference(&seen).copied().collect();
        for id in gone {
            self.present.remove(&id);
            (self.on_event)(DeviceEvent::Removed { id });
        }
    }
}
