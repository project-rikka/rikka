use rikka_types::{Color, Point, Size};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeKey(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FrameKey(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PortSide {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PortAddr {
    pub node: NodeKey,
    pub side: PortSide,
    pub index: usize,
}

impl PortAddr {
    pub fn input(node: NodeKey, index: usize) -> Self {
        Self {
            node,
            side: PortSide::Input,
            index,
        }
    }

    pub fn output(node: NodeKey, index: usize) -> Self {
        Self {
            node,
            side: PortSide::Output,
            index,
        }
    }
}

#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasPort {
    pub label: String,
    pub color: Color,
    pub many: bool,
}

impl CanvasPort {
    pub fn new(label: impl Into<String>, color: Color) -> Self {
        Self {
            label: label.into(),
            color,
            many: false,
        }
    }

    pub fn many(mut self) -> Self {
        self.many = true;
        self
    }
}

#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasNode {
    pub key: NodeKey,
    pub title: String,
    pub position: Point,
    pub width: f32,
    pub accent: Color,
    pub inputs: Vec<CanvasPort>,
    pub outputs: Vec<CanvasPort>,
    pub detail: Option<String>,
    pub swatch: Option<Color>,
    pub problem: Option<String>,
}

pub const NODE_WIDTH: f32 = 176.0;

impl CanvasNode {
    pub fn new(key: NodeKey, title: impl Into<String>, position: Point) -> Self {
        Self {
            key,
            title: title.into(),
            position,
            width: NODE_WIDTH,
            accent: Color::rgb(0.45, 0.5, 0.6),
            inputs: Vec::new(),
            outputs: Vec::new(),
            detail: None,
            swatch: None,
            problem: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn accent(mut self, accent: Color) -> Self {
        self.accent = accent;
        self
    }

    pub fn input(mut self, port: CanvasPort) -> Self {
        self.inputs.push(port);
        self
    }

    pub fn output(mut self, port: CanvasPort) -> Self {
        self.outputs.push(port);
        self
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn swatch(mut self, swatch: Color) -> Self {
        self.swatch = Some(swatch);
        self
    }

    pub fn problem(mut self, problem: impl Into<String>) -> Self {
        self.problem = Some(problem.into());
        self
    }

    pub fn port(&self, side: PortSide, index: usize) -> Option<&CanvasPort> {
        match side {
            PortSide::Input => self.inputs.get(index),
            PortSide::Output => self.outputs.get(index),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CanvasLink {
    pub from: PortAddr,
    pub to: PortAddr,
}

impl CanvasLink {
    pub fn new(from: PortAddr, to: PortAddr) -> Self {
        Self { from, to }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FrameArea {
    Around(Vec<NodeKey>),
    At { position: Point, size: Size },
}

#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasFrame {
    pub key: FrameKey,
    pub title: String,
    pub color: Color,
    pub area: FrameArea,
}

impl CanvasFrame {
    pub fn group(key: FrameKey, title: impl Into<String>, nodes: Vec<NodeKey>) -> Self {
        Self {
            key,
            title: title.into(),
            color: Color::rgb(0.4, 0.55, 0.85),
            area: FrameArea::Around(nodes),
        }
    }

    pub fn comment(key: FrameKey, text: impl Into<String>, position: Point, size: Size) -> Self {
        Self {
            key,
            title: text.into(),
            color: Color::rgb(0.85, 0.75, 0.35),
            area: FrameArea::At { position, size },
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeGraph {
    pub nodes: Vec<CanvasNode>,
    pub links: Vec<CanvasLink>,
    pub frames: Vec<CanvasFrame>,
}
