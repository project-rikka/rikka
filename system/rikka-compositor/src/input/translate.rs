use rikka_compositor_input::{InputEvent, ScrollAxis};
use rikka_compositor_seat::{Axis, AxisMotion};
use rikka_compositor_types::AxisSource;

use super::pending::{Pending, PendingEvent};
use super::rules::PointerRules;
use super::touch;

pub(crate) fn translate(
    pending: &mut Pending,
    rules: &PointerRules,
    event: &InputEvent,
) -> Option<PendingEvent> {
    match event {
        InputEvent::Key {
            device,
            time,
            code,
            press,
            ..
        } => Some(PendingEvent::Key {
            device: pending.device(*device),
            code: *code,
            press: *press,
            time: time.as_monotonic(),
        }),
        InputEvent::PointerButton {
            time, code, press, ..
        } => Some(PendingEvent::Button {
            code: *code,
            press: *press,
            time: time.as_monotonic(),
        }),
        InputEvent::Scroll {
            time,
            source,
            horizontal,
            vertical,
            natural,
            ..
        } => {
            let factor = rules.scroll_factor(*source == AxisSource::Finger);
            let mut axis = Axis::new(time.as_monotonic());
            axis.source = Some(*source);
            axis.horizontal = motion(*horizontal, factor, *natural);
            axis.vertical = motion(*vertical, factor, *natural);
            (!axis.is_empty()).then_some(PendingEvent::Axis(axis))
        }
        InputEvent::Gesture { gesture, .. } => Some(PendingEvent::Gesture(*gesture)),
        InputEvent::Tool {
            device,
            time,
            tool,
            event,
            state,
        } => Some(PendingEvent::Tool {
            device: *device,
            tool: *tool,
            event: *event,
            state: *state,
            time: time.as_monotonic(),
        }),
        InputEvent::Pad {
            device,
            time,
            input,
            group,
            mode,
        } => Some(PendingEvent::Pad {
            device: *device,
            input: *input,
            group: *group,
            mode: *mode,
            time: time.as_monotonic(),
        }),
        InputEvent::DeviceAdded(device) => {
            tracing::info!("input device added: {}", device.name);
            pending.register_device(device);
            Some(PendingEvent::Added((**device).clone()))
        }
        InputEvent::DeviceRemoved(device) => {
            pending.forget_device(*device);
            Some(PendingEvent::Removed(*device))
        }
        event => touch::pending_event(event),
    }
}

fn motion(axis: Option<ScrollAxis>, factor: f64, natural: bool) -> AxisMotion {
    match axis {
        None => AxisMotion::default(),
        Some(ScrollAxis::Stopped) => AxisMotion {
            stopped: true,
            inverted: natural,
            ..AxisMotion::default()
        },
        Some(ScrollAxis::Moved { value, v120 }) => AxisMotion {
            value: value * factor,
            v120: v120.map(|v120| notches(v120 * factor)),
            stopped: false,
            inverted: natural,
        },
    }
}

pub(super) fn scaled(mut axis: Axis, rules: &PointerRules) -> Axis {
    let factor = rules.scroll_factor(axis.source == Some(AxisSource::Finger));
    for motion in [&mut axis.horizontal, &mut axis.vertical] {
        motion.value *= factor;
        motion.v120 = motion.v120.map(|v120| notches(f64::from(v120) * factor));
    }
    axis
}

fn notches(scaled: f64) -> i32 {
    match scaled as i32 {
        0 if scaled > 0.0 => 1,
        0 if scaled < 0.0 => -1,
        notches => notches,
    }
}

impl PointerRules {
    pub(crate) fn scroll_factor(&self, finger: bool) -> f64 {
        match finger {
            true => self.touchpad.scroll_factor,
            false => self.mouse.scroll_factor,
        }
    }
}
