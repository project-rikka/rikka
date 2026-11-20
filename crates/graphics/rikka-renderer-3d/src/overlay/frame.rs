use bytemuck::{Pod, Zeroable};

use crate::error::{Error, Result};
use crate::overlay::batch::{
    OverlayBatch, OverlayDepth, OverlayPrimitive, OverlayShading, OverlaySmoothing, Overlays,
    PointShape, SourceKind,
};
use crate::overlay::pool::{COLOR_BYTES, INDEX_BYTES, OverlayPool, POSITION_BYTES};

const QUAD_CORNERS: u32 = 6;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct GpuOverlayBatch {
    pub transform: [f32; 16],
    pub positions: u64,
    pub colors: u64,
    pub indices: u64,
    pub first: u32,
    pub width: f32,
    pub bias: f32,
    pub dash: f32,
    pub gap: f32,
    pub outline_width: f32,
    pub outline_color: u32,
    pub shape: u32,
    pub shading: u32,
    pub clipped: u32,
    pub aliased: u32,
    pub pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub(crate) struct OverlayPush {
    pub frame: u64,
    pub batch: u64,
    pub alpha: f32,
    pub pad: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverlayKind {
    Lines,
    Points,
    Triangles,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OverlayLayer {
    Scene,
    Front,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OverlayDraw {
    pub kind: OverlayKind,
    pub layer: OverlayLayer,
    pub faded: Option<f32>,
    pub writes_depth: bool,
    pub cull: bool,
    pub vertices: u32,
    pub record: u32,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ImmediateBases {
    pub positions: u64,
    pub colors: u64,
    pub indices: u64,
}

#[derive(Default)]
pub(crate) struct OverlayFrame {
    draws: Vec<OverlayDraw>,
    records: Vec<GpuOverlayBatch>,
    immediate: Vec<Option<bool>>,
}

struct Source {
    positions: u64,
    colors: u64,
    indices: Option<u64>,
    elements: u32,
    immediate: Option<bool>,
}

impl OverlayFrame {
    pub(crate) fn resolve(&mut self, overlays: &Overlays, pool: &OverlayPool) -> Result<()> {
        self.draws.clear();
        self.records.clear();
        self.immediate.clear();
        if let Some(invalid) = &overlays.invalid {
            return Err(Error::Invalid(invalid.clone()));
        }
        for batch in &overlays.batches {
            let source = source(overlays, pool, batch)?;
            let corners = batch.primitive.corners();
            let count = match batch.count {
                Some(count) => count,
                None => source.elements.saturating_sub(batch.first),
            };
            if u64::from(batch.first) + u64::from(count) > u64::from(source.elements) {
                return Err(Error::Invalid(format!(
                    "overlay batch ends at element {} of {}",
                    u64::from(batch.first) + u64::from(count),
                    source.elements
                )));
            }
            if !count.is_multiple_of(corners) {
                return Err(Error::Invalid(format!(
                    "overlay batch draws {count} elements, not a multiple of {corners}"
                )));
            }
            if count == 0 {
                continue;
            }
            let (record, kind, cull) = record(batch, &source)?;
            let vertices = match kind {
                OverlayKind::Lines => count / 2 * QUAD_CORNERS,
                OverlayKind::Points => count * QUAD_CORNERS,
                OverlayKind::Triangles => count,
            };
            let (layer, faded) = match batch.depth {
                OverlayDepth::Tested => (OverlayLayer::Scene, None),
                OverlayDepth::Faded(alpha) => (OverlayLayer::Scene, Some(alpha.clamp(0.0, 1.0))),
                OverlayDepth::Always => (OverlayLayer::Front, None),
            };
            self.draws.push(OverlayDraw {
                kind,
                layer,
                faded,
                writes_depth: batch.writes_depth,
                cull,
                vertices: u32::try_from(u64::from(vertices))
                    .map_err(|_| Error::Invalid("overlay batch is too large".into()))?,
                record: self.records.len() as u32,
            });
            self.records.push(record);
            self.immediate.push(source.immediate);
        }
        Ok(())
    }

    pub(crate) fn place(&mut self, bases: ImmediateBases) {
        for (record, immediate) in self.records.iter_mut().zip(&self.immediate) {
            let Some(indexed) = *immediate else {
                continue;
            };
            record.positions += bases.positions;
            record.colors += bases.colors;
            if indexed {
                record.indices += bases.indices;
            }
        }
    }

    pub(crate) fn records(&self) -> &[GpuOverlayBatch] {
        &self.records
    }

    pub(crate) fn draws(&self) -> &[OverlayDraw] {
        &self.draws
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.draws.is_empty()
    }

    pub(crate) fn tested(&self) -> bool {
        self.draws
            .iter()
            .any(|draw| draw.layer == OverlayLayer::Scene)
    }
}

fn source(overlays: &Overlays, pool: &OverlayPool, batch: &OverlayBatch) -> Result<Source> {
    match batch.source.0 {
        SourceKind::Buffer(id) => {
            let placed = pool.placed(id)?;
            Ok(Source {
                positions: placed.positions,
                colors: placed.colors,
                indices: (placed.index_count > 0).then_some(placed.indices),
                elements: match placed.index_count {
                    0 => placed.vertices,
                    count => count,
                },
                immediate: None,
            })
        }
        SourceKind::Immediate(index) => {
            let Some(&span) = overlays.spans.get(index as usize) else {
                return Err(Error::Invalid(
                    "overlay source names data of another frame".into(),
                ));
            };
            Ok(Source {
                positions: u64::from(span.first_vertex) * POSITION_BYTES,
                colors: u64::from(span.first_vertex) * COLOR_BYTES,
                indices: (span.indices > 0).then(|| u64::from(span.first_index) * INDEX_BYTES),
                elements: match span.indices {
                    0 => span.vertices,
                    count => count,
                },
                immediate: Some(span.indices > 0),
            })
        }
    }
}

fn finite(value: f32, what: &str) -> Result<f32> {
    match value.is_finite() && value >= 0.0 {
        true => Ok(value),
        false => Err(Error::Invalid(format!(
            "overlay {what} {value} is not a finite non-negative number"
        ))),
    }
}

fn record(batch: &OverlayBatch, source: &Source) -> Result<(GpuOverlayBatch, OverlayKind, bool)> {
    let mut record = GpuOverlayBatch {
        transform: batch.transform.to_cols_array(),
        positions: source.positions,
        colors: source.colors,
        indices: source.indices.unwrap_or(0),
        first: batch.first,
        bias: finite(batch.bias, "depth bias")?.min(1.0),
        clipped: u32::from(batch.clipped),
        ..GpuOverlayBatch::default()
    };
    let (kind, cull) = match batch.primitive {
        OverlayPrimitive::Lines(lines) => {
            record.width = finite(lines.width, "line width")?;
            record.aliased = u32::from(lines.smoothing == OverlaySmoothing::Aliased);
            if let Some(dash) = lines.dash {
                record.dash = finite(dash.length, "dash length")?;
                record.gap = finite(dash.gap, "dash gap")?;
            }
            (OverlayKind::Lines, false)
        }
        OverlayPrimitive::Points(points) => {
            record.width = finite(points.size, "point size")?;
            record.aliased = u32::from(points.smoothing == OverlaySmoothing::Aliased);
            record.shape = match points.shape {
                PointShape::Disc => 0,
                PointShape::Square => 1,
            };
            if let Some(outline) = points.outline {
                record.outline_width = finite(outline.width, "point outline width")?;
                record.outline_color = u32::from_le_bytes(outline.color);
            }
            (OverlayKind::Points, false)
        }
        OverlayPrimitive::Triangles(triangles) => {
            record.shading = match triangles.shading {
                OverlayShading::Flat => 0,
                OverlayShading::Facing => 1,
            };
            (OverlayKind::Triangles, triangles.cull_back_faces)
        }
    };
    Ok((record, kind, cull))
}
