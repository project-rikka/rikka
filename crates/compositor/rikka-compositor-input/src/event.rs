use rikka_compositor_geometry::{Logical, Point};
use rikka_compositor_reactor::Timestamp;
use rikka_compositor_types::{AxisSource, Gesture, PadInput, Press, ToolInfo};

use crate::device::{DeviceId, DeviceInfo};
use crate::scroll::ScrollAxis;
use crate::space::Normalized;
use crate::tablet::{ToolEvent, ToolState};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Switch {
    Lid,
    TabletMode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    DeviceAdded(Box<DeviceInfo>),
    DeviceRemoved(DeviceId),
    Key {
        device: DeviceId,
        time: Timestamp,
        code: u32,
        press: Press,
        seat_count: u32,
    },
    PointerMotion {
        device: DeviceId,
        time: Timestamp,
        delta: Point<f64, Logical>,
        unaccelerated: Point<f64, Logical>,
        moved_to: Option<Point<f64, Logical>>,
    },
    PointerMotionAbsolute {
        device: DeviceId,
        time: Timestamp,
        position: Point<f64, Normalized>,
    },
    PointerButton {
        device: DeviceId,
        time: Timestamp,
        code: u32,
        press: Press,
        seat_count: u32,
    },
    Scroll {
        device: DeviceId,
        time: Timestamp,
        source: AxisSource,
        horizontal: Option<ScrollAxis>,
        vertical: Option<ScrollAxis>,
        natural: bool,
    },
    Gesture {
        device: DeviceId,
        gesture: Gesture,
    },
    TouchDown {
        device: DeviceId,
        time: Timestamp,
        slot: u32,
        position: Point<f64, Normalized>,
    },
    TouchMotion {
        device: DeviceId,
        time: Timestamp,
        slot: u32,
        position: Point<f64, Normalized>,
    },
    TouchUp {
        device: DeviceId,
        time: Timestamp,
        slot: u32,
    },
    TouchCancel {
        device: DeviceId,
        time: Timestamp,
    },
    TouchFrame {
        device: DeviceId,
        time: Timestamp,
    },
    Tool {
        device: DeviceId,
        time: Timestamp,
        tool: ToolInfo,
        event: ToolEvent,
        state: ToolState,
    },
    Pad {
        device: DeviceId,
        time: Timestamp,
        input: PadInput,
        group: u32,
        mode: u32,
    },
    PadKey {
        device: DeviceId,
        time: Timestamp,
        code: u32,
        press: Press,
    },
    Switch {
        device: DeviceId,
        time: Timestamp,
        switch: Switch,
        on: bool,
    },
}

const _: () = assert!(size_of::<InputEvent>() <= 112);
