use glam::{Mat4, Vec3};

use crate::error::{Error, Result};
use crate::overlay::pool::OverlayBufferId;

const LINE_BIAS: f32 = 0.00125;
const POINT_BIAS: f32 = 0.001875;
const TRIANGLE_BIAS: f32 = 0.000625;

#[derive(Clone, Copy, Debug, Default)]
pub struct OverlayData<'a> {
    pub positions: &'a [Vec3],
    pub colors: &'a [[u8; 4]],
    pub indices: &'a [u32],
}

impl OverlayData<'_> {
    pub(crate) fn validate(&self) -> Result<()> {
        let vertices = self.positions.len();
        if u32::try_from(vertices).is_err() || u32::try_from(self.indices.len()).is_err() {
            return Err(Error::Invalid(
                "overlay data exceeds 2^32 vertices or indices".into(),
            ));
        }
        if self.colors.len() != vertices {
            return Err(Error::Invalid(format!(
                "overlay data has {} colors for {vertices} positions",
                self.colors.len()
            )));
        }
        check_indices(self.indices, vertices as u32)
    }
}

pub(crate) fn check_indices(indices: &[u32], vertices: u32) -> Result<()> {
    match indices.iter().copied().max() {
        Some(worst) if worst >= vertices => Err(Error::Invalid(format!(
            "overlay index {worst} is out of range for {vertices} vertices"
        ))),
        _ => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct OverlayUpdate<'a> {
    pub first: u32,
    pub positions: &'a [Vec3],
    pub colors: &'a [[u8; 4]],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceKind {
    Buffer(OverlayBufferId),
    Immediate(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlaySource(pub(crate) SourceKind);

impl From<OverlayBufferId> for OverlaySource {
    fn from(buffer: OverlayBufferId) -> Self {
        Self(SourceKind::Buffer(buffer))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum OverlayDepth {
    #[default]
    Tested,
    Faded(f32),
    Always,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dash {
    pub length: f32,
    pub gap: f32,
}

impl Dash {
    pub fn new(length: f32, gap: f32) -> Self {
        Self { length, gap }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PointShape {
    #[default]
    Disc,
    Square,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointOutline {
    pub color: [u8; 4],
    pub width: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlayShading {
    #[default]
    Flat,
    Facing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OverlaySmoothing {
    #[default]
    Smooth,
    Aliased,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct OverlayLines {
    pub width: f32,
    pub dash: Option<Dash>,
    pub smoothing: OverlaySmoothing,
}

impl OverlayLines {
    pub fn new(width: f32) -> Self {
        Self {
            width,
            dash: None,
            smoothing: OverlaySmoothing::Smooth,
        }
    }

    pub fn dash(mut self, dash: Dash) -> Self {
        self.dash = Some(dash);
        self
    }

    pub fn smoothing(mut self, smoothing: OverlaySmoothing) -> Self {
        self.smoothing = smoothing;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct OverlayPoints {
    pub size: f32,
    pub shape: PointShape,
    pub outline: Option<PointOutline>,
    pub smoothing: OverlaySmoothing,
}

impl OverlayPoints {
    pub fn new(size: f32) -> Self {
        Self {
            size,
            shape: PointShape::Disc,
            outline: None,
            smoothing: OverlaySmoothing::Smooth,
        }
    }

    pub fn shape(mut self, shape: PointShape) -> Self {
        self.shape = shape;
        self
    }

    pub fn outline(mut self, color: [u8; 4], width: f32) -> Self {
        self.outline = Some(PointOutline { color, width });
        self
    }

    pub fn smoothing(mut self, smoothing: OverlaySmoothing) -> Self {
        self.smoothing = smoothing;
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct OverlayTriangles {
    pub shading: OverlayShading,
    pub cull_back_faces: bool,
}

impl OverlayTriangles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn shading(mut self, shading: OverlayShading) -> Self {
        self.shading = shading;
        self
    }

    pub fn cull_back_faces(mut self, cull: bool) -> Self {
        self.cull_back_faces = cull;
        self
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverlayPrimitive {
    Lines(OverlayLines),
    Points(OverlayPoints),
    Triangles(OverlayTriangles),
}

impl OverlayPrimitive {
    fn bias(self) -> f32 {
        match self {
            Self::Lines(_) => LINE_BIAS,
            Self::Points(_) => POINT_BIAS,
            Self::Triangles(_) => TRIANGLE_BIAS,
        }
    }

    pub(crate) fn corners(self) -> u32 {
        match self {
            Self::Lines(_) => 2,
            Self::Points(_) => 1,
            Self::Triangles(_) => 3,
        }
    }
}

impl From<OverlayLines> for OverlayPrimitive {
    fn from(lines: OverlayLines) -> Self {
        Self::Lines(lines)
    }
}

impl From<OverlayPoints> for OverlayPrimitive {
    fn from(points: OverlayPoints) -> Self {
        Self::Points(points)
    }
}

impl From<OverlayTriangles> for OverlayPrimitive {
    fn from(triangles: OverlayTriangles) -> Self {
        Self::Triangles(triangles)
    }
}

#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct OverlayBatch {
    pub source: OverlaySource,
    pub primitive: OverlayPrimitive,
    pub first: u32,
    pub count: Option<u32>,
    pub transform: Mat4,
    pub depth: OverlayDepth,
    pub bias: f32,
    pub writes_depth: bool,
    pub clipped: bool,
}

impl OverlayBatch {
    pub fn new(source: impl Into<OverlaySource>, primitive: impl Into<OverlayPrimitive>) -> Self {
        let primitive = primitive.into();
        Self {
            source: source.into(),
            primitive,
            first: 0,
            count: None,
            transform: Mat4::IDENTITY,
            depth: OverlayDepth::Tested,
            bias: primitive.bias(),
            writes_depth: false,
            clipped: true,
        }
    }

    pub fn range(mut self, first: u32, count: u32) -> Self {
        self.first = first;
        self.count = Some(count);
        self
    }

    pub fn transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
        self
    }

    pub fn depth(mut self, depth: OverlayDepth) -> Self {
        self.depth = depth;
        self
    }

    pub fn bias(mut self, bias: f32) -> Self {
        self.bias = bias;
        self
    }

    pub fn writes_depth(mut self, writes: bool) -> Self {
        self.writes_depth = writes;
        self
    }

    pub fn clipped(mut self, clipped: bool) -> Self {
        self.clipped = clipped;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ImmediateSpan {
    pub first_vertex: u32,
    pub vertices: u32,
    pub first_index: u32,
    pub indices: u32,
}

#[derive(Debug, Default)]
pub(crate) struct Overlays {
    pub batches: Vec<OverlayBatch>,
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<[u8; 4]>,
    pub indices: Vec<u32>,
    pub spans: Vec<ImmediateSpan>,
    pub invalid: Option<String>,
}

impl Overlays {
    pub(crate) fn add(&mut self, data: &OverlayData<'_>) -> OverlaySource {
        if let Err(error) = data.validate() {
            self.invalid.get_or_insert_with(|| error.to_string());
        }
        let span = ImmediateSpan {
            first_vertex: self.positions.len() as u32,
            vertices: data.positions.len() as u32,
            first_index: self.indices.len() as u32,
            indices: data.indices.len() as u32,
        };
        self.positions
            .extend(data.positions.iter().map(|position| position.to_array()));
        self.colors.extend_from_slice(data.colors);
        self.colors.resize(self.positions.len(), [255; 4]);
        self.indices.extend_from_slice(data.indices);
        self.spans.push(span);
        OverlaySource(SourceKind::Immediate(self.spans.len() as u32 - 1))
    }

    pub(crate) fn occluded(&self) -> bool {
        self.batches
            .iter()
            .any(|batch| batch.depth != OverlayDepth::Always)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }
}
