use std::fmt;
use std::os::fd::AsFd;
use std::rc::Rc;

use rikka_compositor_reactor::{Interest, Reactor, Token};
use udev::EventType;

use crate::error::{Error, Result};
use crate::gpu::{Gpu, is_card, on_seat};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HotplugEvent {
    Added(Gpu),
    Changed(u64),
    Removed(u64),
}

pub struct Hotplug<S: 'static> {
    reactor: Reactor<S>,
    token: Token,
}

impl<S: 'static> Hotplug<S> {
    pub fn watch(
        reactor: &Reactor<S>,
        seat: &str,
        mut on: impl FnMut(&mut S, HotplugEvent) + 'static,
    ) -> Result<Self> {
        let socket = udev::MonitorBuilder::new()
            .and_then(|builder| builder.match_subsystem("drm"))
            .and_then(|builder| builder.listen())
            .map_err(Error::Udev)?;
        let seat = seat.to_owned();
        let socket = Rc::new(socket);
        let listening = Rc::clone(&socket);
        let token = reactor
            .watch(socket.as_fd(), Interest::READABLE, move |state, _| {
                for event in listening.iter() {
                    let Some(number) = event.devnum() else {
                        continue;
                    };
                    if !is_card(&event) || !on_seat(&event, &seat) {
                        continue;
                    }
                    let hotplug_event = match event.event_type() {
                        EventType::Change => HotplugEvent::Changed(number),
                        EventType::Remove => HotplugEvent::Removed(number),
                        EventType::Add => match Gpu::from_udev(&event.device()) {
                            Some(gpu) => HotplugEvent::Added(gpu),
                            None => continue,
                        },
                        _ => continue,
                    };
                    on(state, hotplug_event);
                }
            })
            .map_err(Error::Reactor)?;
        Ok(Self {
            reactor: reactor.clone(),
            token,
        })
    }
}

impl<S: 'static> Drop for Hotplug<S> {
    fn drop(&mut self) {
        self.reactor.remove(self.token);
    }
}

impl<S: 'static> fmt::Debug for Hotplug<S> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Hotplug")
            .field("token", &self.token)
            .finish()
    }
}
