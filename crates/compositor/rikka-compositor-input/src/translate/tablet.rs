use input::event::tablet_pad::{
    KeyState, RingAxisSource, StripAxisSource, TabletPadEvent, TabletPadEventTrait,
};
use input::event::tablet_tool::{
    ProximityState, TabletTool, TabletToolEvent, TabletToolEventTrait, TabletToolType, TipState,
};
use input::{AsRaw, ffi};
use rikka_compositor_geometry::Point;
use rikka_compositor_reactor::Timestamp;
use rikka_compositor_types::{PadInput, PadSource, Press, ToolCapabilities, ToolInfo, ToolKind};

use super::press;
use crate::device::DeviceId;
use crate::event::InputEvent;
use crate::tablet::{Changed, ToolEvent, ToolState};

const LIFTED: f64 = -1.0;

#[derive(Debug, Default)]
pub(crate) struct Tools {
    known: Vec<(TabletTool, ToolInfo)>,
}

impl Tools {
    pub(crate) fn forget(&mut self) {
        self.known.clear();
    }

    fn describe(&mut self, event: &TabletToolEvent) -> ToolInfo {
        let raw = unsafe { ffi::libinput_event_tablet_tool_get_tool(event.as_raw_mut()) };
        if let Some((_, tool)) = self
            .known
            .iter()
            .find(|(tracked, _)| tracked.as_raw() == raw.cast_const())
        {
            return *tool;
        }
        let tracked = event.tool();
        let tool = describe(&tracked);
        self.known.push((tracked, tool));
        tool
    }
}

pub(super) fn tool(
    event: TabletToolEvent,
    device: DeviceId,
    tools: &mut Tools,
) -> Option<InputEvent> {
    let time = Timestamp::from_micros(event.time_usec());
    let tool = tools.describe(&event);
    let (tool_event, entering) = match &event {
        TabletToolEvent::Proximity(proximity) => match proximity.proximity_state() {
            ProximityState::In => (ToolEvent::ProximityIn, true),
            ProximityState::Out => (ToolEvent::ProximityOut, false),
        },
        TabletToolEvent::Axis(_) => (ToolEvent::Motion, false),
        TabletToolEvent::Tip(tip) => (
            ToolEvent::Tip(match tip.tip_state() {
                TipState::Down => Press::Pressed,
                TipState::Up => Press::Released,
            }),
            false,
        ),
        TabletToolEvent::Button(button) => (
            ToolEvent::Button {
                code: button.button(),
                press: press(button.button_state()),
            },
            false,
        ),
        _ => return None,
    };
    Some(InputEvent::Tool {
        device,
        time,
        tool,
        event: tool_event,
        state: state(&event, tool.capabilities, entering),
    })
}

pub(super) fn pad(event: TabletPadEvent, device: DeviceId) -> Option<InputEvent> {
    let time = Timestamp::from_micros(event.time_usec());
    let group = event.mode_group().index();
    let mode = event.mode();
    let input = match event {
        TabletPadEvent::Button(button) => PadInput::Button {
            button: button.button_number(),
            press: press(button.button_state()),
        },
        TabletPadEvent::Ring(ring) => PadInput::Ring {
            ring: ring.number(),
            degrees: touching(ring.position()),
            source: match ring.source() {
                RingAxisSource::Finger => PadSource::Finger,
                RingAxisSource::Unknown => PadSource::Unknown,
            },
        },
        TabletPadEvent::Strip(strip) => PadInput::Strip {
            strip: strip.number(),
            position: touching(strip.position()),
            source: match strip.source() {
                StripAxisSource::Finger => PadSource::Finger,
                StripAxisSource::Unknown => PadSource::Unknown,
            },
        },
        TabletPadEvent::Key(key) => {
            return Some(InputEvent::PadKey {
                device,
                time,
                code: key.key(),
                press: match key.key_state() {
                    KeyState::Pressed => Press::Pressed,
                    KeyState::Released => Press::Released,
                },
            });
        }
        _ => return None,
    };
    Some(InputEvent::Pad {
        device,
        time,
        input,
        group,
        mode,
    })
}

fn touching(position: f64) -> Option<f64> {
    (position != LIFTED).then_some(position)
}

fn describe(tool: &TabletTool) -> ToolInfo {
    let capabilities = [
        (tool.has_pressure(), ToolCapabilities::PRESSURE),
        (tool.has_distance(), ToolCapabilities::DISTANCE),
        (tool.has_tilt(), ToolCapabilities::TILT),
        (tool.has_rotation(), ToolCapabilities::ROTATION),
        (tool.has_slider(), ToolCapabilities::SLIDER),
        (tool.has_wheel(), ToolCapabilities::WHEEL),
    ]
    .into_iter()
    .filter(|(has, _)| *has)
    .fold(ToolCapabilities::empty(), |all, (_, flag)| all | flag);
    ToolInfo {
        kind: match tool.tool_type() {
            Some(TabletToolType::Eraser) => ToolKind::Eraser,
            Some(TabletToolType::Brush) => ToolKind::Brush,
            Some(TabletToolType::Pencil) => ToolKind::Pencil,
            Some(TabletToolType::Airbrush) => ToolKind::Airbrush,
            Some(TabletToolType::Mouse) => ToolKind::Mouse,
            Some(TabletToolType::Lens) => ToolKind::Lens,
            _ => ToolKind::Pen,
        },
        serial: tool.serial(),
        hardware_id: tool.tool_id(),
        capabilities,
    }
}

fn state(
    event: &impl TabletToolEventTrait,
    capabilities: ToolCapabilities,
    entering: bool,
) -> ToolState {
    let has = |capability| capabilities.contains(capability);
    let value = |capability, read: &dyn Fn() -> f64| {
        if has(capability) { read() as f32 } else { 0.0 }
    };
    let mut changed = Changed::empty();
    for (axis, capability, read) in [
        (
            Changed::PRESSURE,
            ToolCapabilities::PRESSURE,
            &(|| event.pressure_has_changed()) as &dyn Fn() -> bool,
        ),
        (Changed::DISTANCE, ToolCapabilities::DISTANCE, &|| {
            event.distance_has_changed()
        }),
        (Changed::TILT, ToolCapabilities::TILT, &|| {
            event.tilt_x_has_changed() || event.tilt_y_has_changed()
        }),
        (Changed::ROTATION, ToolCapabilities::ROTATION, &|| {
            event.rotation_has_changed()
        }),
        (Changed::SLIDER, ToolCapabilities::SLIDER, &|| {
            event.slider_has_changed()
        }),
        (Changed::WHEEL, ToolCapabilities::WHEEL, &|| {
            event.wheel_has_changed()
        }),
    ] {
        let moved = match entering {
            true => axis != Changed::WHEEL,
            false => read(),
        };
        changed.set(axis, has(capability) && moved);
    }
    ToolState {
        position: Point::new(event.x_transformed(1), event.y_transformed(1)),
        pressure: value(ToolCapabilities::PRESSURE, &|| event.pressure()),
        distance: value(ToolCapabilities::DISTANCE, &|| event.distance()),
        tilt: (
            value(ToolCapabilities::TILT, &|| event.tilt_x()),
            value(ToolCapabilities::TILT, &|| event.tilt_y()),
        ),
        rotation: value(ToolCapabilities::ROTATION, &|| event.rotation()),
        slider: value(ToolCapabilities::SLIDER, &|| event.slider_position()),
        wheel: value(ToolCapabilities::WHEEL, &|| event.wheel_delta()),
        wheel_steps: value(ToolCapabilities::WHEEL, &|| event.wheel_delta_discrete()) as i32,
        changed,
    }
}
