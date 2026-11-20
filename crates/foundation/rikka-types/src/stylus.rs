use std::time::Duration;

use crate::point::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

impl ToolKind {
    #[must_use]
    pub fn is_eraser(self) -> bool {
        self == ToolKind::Eraser
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ToolCapabilities {
    pub pressure: bool,
    pub tilt: bool,
    pub distance: bool,
    pub rotation: bool,
    pub slider: bool,
    pub wheel: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tool {
    pub kind: ToolKind,
    pub serial: u64,
    pub hardware_id: u64,
    pub capabilities: ToolCapabilities,
}

impl Tool {
    #[must_use]
    pub fn new(kind: ToolKind) -> Tool {
        Tool {
            kind,
            serial: 0,
            hardware_id: 0,
            capabilities: ToolCapabilities::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tilt {
    pub x: f32,
    pub y: f32,
}

impl Tilt {
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Tilt {
        Tilt { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StylusSample {
    pub position: Point,
    pub pressure: Option<f32>,
    pub distance: Option<f32>,
    pub tilt: Option<Tilt>,
    pub rotation: Option<f32>,
    pub slider: Option<f32>,
    pub contact: bool,
    pub time: Duration,
}

impl StylusSample {
    #[must_use]
    pub fn new(position: Point) -> StylusSample {
        StylusSample {
            position,
            pressure: None,
            distance: None,
            tilt: None,
            rotation: None,
            slider: None,
            contact: false,
            time: Duration::ZERO,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Stylus {
    pub tool: Tool,
    samples: Vec<StylusSample>,
}

impl Stylus {
    #[must_use]
    pub fn new(tool: Tool, sample: StylusSample) -> Stylus {
        Stylus {
            tool,
            samples: vec![sample],
        }
    }

    pub fn push(&mut self, sample: StylusSample) {
        self.samples.push(sample);
    }

    #[must_use]
    pub fn sample(&self) -> &StylusSample {
        self.samples
            .last()
            .expect("a stylus holds at least one sample")
    }

    #[must_use]
    pub fn samples(&self) -> &[StylusSample] {
        &self.samples
    }

    #[must_use]
    pub fn pressure(&self) -> Option<f32> {
        self.sample().pressure
    }

    #[must_use]
    pub fn is_eraser(&self) -> bool {
        self.tool.kind.is_eraser()
    }

    pub fn map_positions(&mut self, mut map: impl FnMut(Point) -> Point) {
        for sample in &mut self.samples {
            sample.position = map(sample.position);
        }
    }
}
