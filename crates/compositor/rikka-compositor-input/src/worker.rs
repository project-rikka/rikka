use std::io;
use std::os::fd::AsFd;
use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use input::event::{DeviceEvent, Event, EventTrait};
use input::{Device, Libinput};
use rikka_compositor_reactor::{Interest, Reactor, Sender};

use crate::access::DeviceAccess;
use crate::apply::configure;
use crate::cursor::{CursorFollow, CursorSink};
use crate::devices::{Devices, TrackedDevice, describe};
use crate::error::Error;
use crate::event::InputEvent;
use crate::leds::Leds;
use crate::notice::InputNotice;
use crate::queue::InputQueue;
use crate::report::ConfigReport;
use crate::route::PointerRoute;
use crate::settings::DeviceSettings;
use crate::shared::PointerShared;
use crate::thread::Control;
use crate::translate::{Tools, translate};

pub(crate) struct Setup {
    pub(crate) seat: String,
    pub(crate) queue: InputQueue,
    pub(crate) notices: Sender<InputNotice>,
    pub(crate) pointer: Arc<PointerShared>,
    pub(crate) access: DeviceAccess,
    pub(crate) settings: Option<DeviceSettings>,
    pub(crate) cursor: Option<Box<dyn CursorSink>>,
}

pub(crate) fn run(setup: Setup, ready: SyncSender<io::Result<Sender<Control>>>) {
    let reactor = match Reactor::<Worker>::new() {
        Ok(reactor) => reactor,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let control = match reactor.mailbox(Worker::control) {
        Ok(control) => control,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if ready.send(Ok(control)).is_err() {
        return;
    }

    let Setup {
        seat,
        queue,
        notices,
        pointer,
        access,
        settings,
        cursor,
    } = setup;
    let mut libinput = Libinput::new_with_udev(access);
    if libinput.udev_assign_seat(&seat).is_err() {
        let _ = notices.send(InputNotice::Failed(Error::Seat(seat)));
        return;
    }
    let mut worker = Worker {
        libinput,
        devices: Devices::default(),
        tools: Tools::default(),
        settings,
        leds: Leds::empty(),
        pointer,
        cursor: CursorFollow::new(cursor),
        queue,
        pending: Vec::new(),
        notices,
        stopped: false,
    };
    let watched = reactor.watch(worker.libinput.as_fd(), Interest::READABLE, |worker, _| {
        worker.dispatch();
    });
    let outcome = watched.and_then(|_| {
        worker.dispatch();
        reactor.run(&mut worker, |worker| worker.stopped)
    });
    if let Err(error) = outcome {
        worker.notify(InputNotice::Failed(Error::Reactor(error)));
    }
}

struct Worker {
    libinput: Libinput,
    devices: Devices,
    tools: Tools,
    settings: Option<DeviceSettings>,
    leds: Leds,
    pointer: Arc<PointerShared>,
    cursor: CursorFollow,
    queue: InputQueue,
    pending: Vec<InputEvent>,
    notices: Sender<InputNotice>,
    stopped: bool,
}

impl Worker {
    fn control(&mut self, control: Control) {
        match control {
            Control::Configure(settings) => {
                self.settings = Some(settings);
                let reports: Vec<_> = self
                    .devices
                    .iter_mut()
                    .filter_map(|tracked| configure_tracked(tracked, &settings))
                    .collect();
                for report in reports {
                    self.notify(InputNotice::Configured(report));
                }
            }
            Control::Leds(leds) => {
                self.leds = leds;
                for tracked in self.devices.iter_mut().filter(|tracked| tracked.keyboard) {
                    tracked.device.led_update(leds.libinput());
                }
            }
            Control::Suspend => {
                self.libinput.suspend();
                self.dispatch();
            }
            Control::Resume => {
                if self.libinput.resume().is_err() {
                    tracing::warn!("failed to resume libinput");
                }
                self.dispatch();
            }
            Control::Stop => self.stopped = true,
        }
    }

    fn dispatch(&mut self) {
        if let Err(error) = self.libinput.dispatch() {
            tracing::warn!("failed to dispatch libinput: {error}");
        }
        let route = self.pointer.current_route();
        let mut reports = Vec::new();
        while let Some(event) = self.libinput.next() {
            match event {
                Event::Device(DeviceEvent::Added(added)) => {
                    let (info, report) = self.added(added.device());
                    self.pending.push(info);
                    reports.extend(report);
                }
                Event::Device(DeviceEvent::Removed(removed)) => {
                    self.tools.forget();
                    if let Some(id) = self.devices.remove(&removed.device()) {
                        self.pending.push(InputEvent::DeviceRemoved(id));
                    }
                }
                event => {
                    let Some(tracked) = self.devices.for_event(&event) else {
                        continue;
                    };
                    if let Some(mut translated) = translate(event, tracked, &mut self.tools) {
                        self.move_pointer(&route, &mut translated);
                        self.pending.push(translated);
                    }
                }
            }
        }
        self.cursor.flush();
        self.queue.push(&mut self.pending);
        for report in reports {
            self.notify(InputNotice::Configured(report));
        }
    }

    fn added(&mut self, device: Device) -> (InputEvent, Option<ConfigReport>) {
        let leds = self.leds;
        let settings = self.settings;
        let tracked = self.devices.add(device);
        if tracked.keyboard {
            tracked.device.led_update(leds.libinput());
        }
        let report = settings.and_then(|settings| configure_tracked(tracked, &settings));
        tracing::debug!("input device added: {}", tracked.device.name());
        (InputEvent::DeviceAdded(Box::new(describe(tracked))), report)
    }

    fn move_pointer(&mut self, route: &PointerRoute, event: &mut InputEvent) {
        let InputEvent::PointerMotion {
            delta, moved_to, ..
        } = event
        else {
            return;
        };
        if let Some((position, output)) = self.pointer.move_pointer(route, *delta) {
            self.cursor.moved(position, output);
            *moved_to = Some(position);
        }
    }

    fn notify(&mut self, notice: InputNotice) {
        if self.notices.send(notice).is_err() {
            self.stopped = true;
        }
    }
}

fn configure_tracked(
    tracked: &mut TrackedDevice,
    settings: &DeviceSettings,
) -> Option<ConfigReport> {
    let kind = tracked.pointer?;
    let report = configure(&mut tracked.device, tracked.id, kind, settings);
    tracked.refresh_natural_scroll();
    Some(report)
}
