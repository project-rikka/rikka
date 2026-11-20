use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_input::{DeviceId, DeviceInfo, Normalized, ToolEvent, ToolState};
use rikka_compositor_seat::{Axis, InputDevice};
use rikka_compositor_types::{Gesture, PadInput, Press, ToolInfo};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PendingEvent {
    Key {
        device: InputDevice,
        code: u32,
        press: Press,
        time: Duration,
    },
    Motion {
        to: Point<f64, Logical>,
        moved: Option<Moved>,
        placed: bool,
        time: Duration,
    },
    Button {
        code: u32,
        press: Press,
        time: Duration,
    },
    Axis(Axis),
    Gesture(Gesture),
    Touch {
        touched: Touched,
        time: Duration,
    },
    Tool {
        device: DeviceId,
        tool: ToolInfo,
        event: ToolEvent,
        state: ToolState,
        time: Duration,
    },
    Pad {
        device: DeviceId,
        input: PadInput,
        group: u32,
        mode: u32,
        time: Duration,
    },
    Added(DeviceInfo),
    Removed(DeviceId),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Moved {
    pub(crate) delta: Point<f64, Logical>,
    pub(crate) unaccelerated: Point<f64, Logical>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AbsoluteSource {
    Device(DeviceId),
    Host,
    Virtual,
    Injected,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Spot {
    At(Point<f64, Logical>),
    On {
        device: DeviceId,
        position: Point<f64, Normalized>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Touched {
    Down { slot: u32, spot: Spot },
    Motion { slot: u32, spot: Spot },
    Up { slot: u32 },
    Cancel,
    Frame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Known {
    input: InputDevice,
    output: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Pending {
    events: VecDeque<PendingEvent>,
    devices: HashMap<DeviceId, Known>,
    named: u64,
    pointed: HashMap<AbsoluteSource, Point<f64, Logical>>,
}

impl Pending {
    pub(crate) fn push(&mut self, event: PendingEvent) {
        self.events.push_back(event);
    }

    pub(crate) fn push_pointed(
        &mut self,
        source: AbsoluteSource,
        to: Point<f64, Logical>,
        time: Duration,
    ) {
        let moved = self.pointed.insert(source, to).and_then(|from| {
            let delta = to - from;
            (delta.x != 0.0 || delta.y != 0.0).then_some(Moved {
                delta,
                unaccelerated: delta,
            })
        });
        self.push(PendingEvent::Motion {
            to,
            moved,
            placed: false,
            time,
        });
    }

    pub(crate) fn take(&mut self) -> VecDeque<PendingEvent> {
        std::mem::take(&mut self.events)
    }

    pub(crate) fn device(&mut self, device: DeviceId) -> InputDevice {
        self.known(device).input
    }

    pub(crate) fn register_device(&mut self, device: &DeviceInfo) {
        self.known(device.id).output.clone_from(&device.output);
    }

    pub(crate) fn output_of(&self, device: DeviceId) -> Option<&str> {
        self.devices.get(&device)?.output.as_deref()
    }

    pub(crate) fn forget_device(&mut self, device: DeviceId) {
        self.devices.remove(&device);
        self.pointed.remove(&AbsoluteSource::Device(device));
    }

    fn known(&mut self, device: DeviceId) -> &mut Known {
        let named = &mut self.named;
        self.devices.entry(device).or_insert_with(|| {
            *named += 1;
            Known {
                input: InputDevice(*named),
                output: None,
            }
        })
    }
}
