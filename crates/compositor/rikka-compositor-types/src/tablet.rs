use crate::input::Press;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolKind {
    Pen,
    Eraser,
    Brush,
    Pencil,
    Airbrush,
    Finger,
    Mouse,
    Lens,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ToolCapabilities: u8 {
        const TILT = 1;
        const PRESSURE = 1 << 1;
        const DISTANCE = 1 << 2;
        const ROTATION = 1 << 3;
        const SLIDER = 1 << 4;
        const WHEEL = 1 << 5;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToolInfo {
    pub kind: ToolKind,
    pub serial: u64,
    pub hardware_id: u64,
    pub capabilities: ToolCapabilities,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ToolAxes {
    pub pressure: Option<f64>,
    pub distance: Option<f64>,
    pub tilt: Option<(f64, f64)>,
    pub rotation: Option<f64>,
    pub slider: Option<f64>,
    pub wheel: Option<(f64, i32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PadSource {
    Finger,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PadInput {
    Button {
        button: u32,
        press: Press,
    },
    Ring {
        ring: u32,
        degrees: Option<f64>,
        source: PadSource,
    },
    Strip {
        strip: u32,
        position: Option<f64>,
        source: PadSource,
    },
    Dial {
        dial: u32,
        v120: i32,
    },
    Mode {
        group: u32,
        mode: u32,
    },
}
