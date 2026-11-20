use std::path::PathBuf;

use paint_document::{Backdrop, Combine, FloodMode, Modify, Shape, StackOrder};
use rikka_figure::TextAlign;
use rikka_image_edit::{
    Anchor, Flip, GradientInk, GradientShape, Intent, PathMark, Profile, QuarterTurn, Recipe,
    Resampling, ShapeStyle, Tool as MarkTool,
};
use rikka_psd::BlendMode;
use rikka_types::{Color, Point};

use crate::masking::MaskOrder;

#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub document: Option<String>,
    pub request: Request,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    Document,
    View(ViewOrder),
    Measure(MeasureOrder),
    History(HistoryOrder),
    Save(SaveOrder),
    Open(PathBuf),
    New(NewOrder),
    Change { name: String, change: Change },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Change {
    Adjust(Vec<AdjustStep>),
    Draw(DrawOrder),
    Select(Vec<SelectStep>),
    Layers(Vec<LayerStep>),
    Transform(Vec<TransformStep>),
    Objects(Vec<ObjectStep>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayerRef {
    Index(u32),
    Name(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Grid {
    Auto,
    Every(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ViewOrder {
    pub region: Option<Rect>,
    pub longest: u32,
    pub layer: Option<LayerRef>,
    pub grid: Option<Grid>,
    pub selection: Option<bool>,
    pub compare: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Chosen,
    Whole,
    Selection,
    Region(Rect),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeasureOrder {
    pub area: Option<Area>,
    pub layer: Option<LayerRef>,
    pub points: Vec<(u32, u32)>,
    pub radius: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewOrder {
    pub width: u32,
    pub height: u32,
    pub resolution: f64,
    pub backdrop: Backdrop,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AdjustStep {
    pub recipe: Recipe,
    pub layer: Option<LayerRef>,
    pub mask: Option<MaskOrder>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DrawOrder {
    pub layer: Option<LayerRef>,
    pub items: Vec<DrawKind>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DrawKind {
    Mark(MarkOrder),
    Brush(BrushOrder),
    Path(PathOrder),
    Gradient(GradientOrder),
    Text(TextOrder),
    Bucket(FloodOrder),
    Fill(Option<Color>),
    Erase(Option<Color>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MarkOrder {
    pub tool: MarkTool,
    pub strokes: Vec<Vec<Point>>,
    pub color: Option<Color>,
    pub width: Option<f32>,
    pub filling: Option<Filling>,
    pub angle: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filling {
    Outline,
    Fill,
    Both(Option<Color>),
}

impl Filling {
    #[must_use]
    pub fn style(self, fallback: Color) -> ShapeStyle {
        match self {
            Filling::Outline => ShapeStyle::Outline,
            Filling::Fill => ShapeStyle::Fill,
            Filling::Both(fill) => ShapeStyle::FillAndOutline {
                fill: fill.unwrap_or(fallback),
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrushOrder {
    pub erases: bool,
    pub strokes: Vec<Vec<Point>>,
    pub size: Option<f32>,
    pub hardness: Option<f32>,
    pub opacity: Option<f32>,
    pub flow: Option<f32>,
    pub antialias: Option<bool>,
    pub color: Option<Color>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PathOrder {
    pub figure: PathMark,
    pub antialias: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GradientOrder {
    pub shape: GradientShape,
    pub from: Point,
    pub to: Point,
    pub ink: Option<GradientInk>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placing {
    At { point: (f32, f32), anchor: Anchor },
    Inside { rect: [f32; 4], anchor: Anchor },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextOrder {
    pub text: String,
    pub font: Option<String>,
    pub size: Option<f32>,
    pub weight: Option<u16>,
    pub italic: Option<bool>,
    pub align: Option<TextAlign>,
    pub wrap: Option<u32>,
    pub antialias: Option<bool>,
    pub color: Option<Color>,
    pub place: Placing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sample {
    Image,
    Layer,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloodOrder {
    pub at: (f32, f32),
    pub tolerance: Option<f32>,
    pub mode: Option<FloodMode>,
    pub sample: Option<Sample>,
    pub antialias: Option<bool>,
    pub color: Option<Color>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SelectStep {
    Shape {
        shape: Shape,
        combine: Combine,
    },
    Wand {
        flood: FloodOrder,
        combine: Combine,
    },
    Layer {
        layer: Option<LayerRef>,
        combine: Combine,
    },
    All,
    Nothing,
    Invert,
    Modify(Modify),
    Move {
        dx: i32,
        dy: i32,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Moving {
    Above(LayerRef),
    Below(LayerRef),
    Into(LayerRef),
    By(i32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct LayerLook {
    pub name: Option<String>,
    pub visible: Option<bool>,
    pub opacity: Option<f32>,
    pub blend: Option<BlendMode>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LayerStep {
    New {
        name: String,
        above: Option<LayerRef>,
    },
    Duplicate {
        layer: Option<LayerRef>,
        name: Option<String>,
    },
    Delete {
        layer: Option<LayerRef>,
    },
    Move {
        layer: Option<LayerRef>,
        to: Moving,
    },
    Restyle {
        layer: Option<LayerRef>,
        look: LayerLook,
    },
    MergeDown {
        layer: Option<LayerRef>,
    },
    MergeVisible,
    Flatten,
    FromBackground {
        name: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum TransformStep {
    Crop(Cropping),
    Resize {
        width: Option<u32>,
        height: Option<u32>,
        resampling: Resampling,
    },
    CanvasSize {
        width: Option<u32>,
        height: Option<u32>,
        anchor: Anchor,
        fill: Option<Color>,
    },
    Rotate {
        degrees: f32,
        resampling: Resampling,
        fill: Option<Color>,
    },
    Turn(QuarterTurn),
    Flip(Flip),
    Convert {
        profile: Profile,
        intent: Intent,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cropping {
    Rect(Rect),
    Selection,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectStep {
    Move {
        objects: Vec<u64>,
        offset: Point,
    },
    Rotate {
        objects: Vec<u64>,
        degrees: f32,
        around: Option<Point>,
    },
    Set {
        object: u64,
        item: DrawKind,
    },
    Delete {
        objects: Vec<u64>,
    },
    Arrange {
        objects: Vec<u64>,
        stack: StackOrder,
    },
    Duplicate {
        objects: Vec<u64>,
        offset: Point,
    },
    Rasterize {
        layer: Option<LayerRef>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryOrder {
    List,
    Undo(u32),
    Redo(u32),
    Revisit(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaveOrder {
    pub path: Option<PathBuf>,
    pub quality: Option<u8>,
}
