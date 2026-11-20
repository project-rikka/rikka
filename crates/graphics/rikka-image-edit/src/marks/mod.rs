mod brush;
mod mark;
mod marking;
mod outline;
mod path;
mod path_mark;
mod shapes;
mod sketch;
mod style;
mod tool;

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use rikka_gpu::{ComputedFrame, ImageFormat};
use rikka_types::{Color, Point};

use crate::error::Result;
use crate::imaging::Imaging;
use crate::ops::{Effect, Reach};
use crate::picture::{Frame, shown_parts};
use crate::plane::Plane;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

pub use brush::{Brush, BrushStroke, Ink};
pub use mark::Mark;
pub use marking::Marking;
pub use path::{Path, PathBuilder, Subpath};
pub use path_mark::{Cap, FillRule, Join, PathMark, StrokeStyle};
pub use sketch::{RemovedMark, Sketch};
pub use style::ShapeStyle;
pub use tool::Tool;

use shapes::{Erasure, Shapes, TILE, bin_shapes};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Checkerboard {
    pub light: Color,
    pub dark: Color,
    pub square: f32,
    pub origin: Point,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Erasing {
    Background,
    Checkerboard(Checkerboard),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Marks {
    marks: Arc<[Marking]>,
}

impl Marks {
    #[must_use]
    pub fn new<M: Into<Marking>>(marks: impl IntoIterator<Item = M>) -> Marks {
        Marks {
            marks: marks.into_iter().map(Into::into).collect(),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    #[must_use]
    pub fn marks(&self) -> &[Marking] {
        &self.marks
    }
}

const OVER: u32 = 0;
const LAYER: u32 = 1;
const ERASED_CHECKERBOARD: Checkerboard = Checkerboard {
    light: Color::TRANSPARENT,
    dark: Color::TRANSPARENT,
    square: 1.0,
    origin: Point::ZERO,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MarksPush {
    head: PushHeader,
    shapes: u64,
    inks: u64,
    tiles: u64,
    entries: u64,
    across: u32,
    light: u32,
    dark: u32,
    square: f32,
    anchor: [f32; 2],
}

fn packed(color: Color) -> u32 {
    u32::from_le_bytes(color.to_rgba8())
}

fn draw(
    work: &mut Work,
    head: PushHeader,
    io: Io<'_>,
    extent: [u32; 2],
    shapes: &Shapes,
    reveal: Checkerboard,
) -> Result<()> {
    let geometry = work.constants(&shapes.shapes)?;
    let inks = work.constants(&shapes.inks)?;
    let tiles = work.constants(&shapes.tiles)?;
    let entries = work.constants(&shapes.entries)?;
    let push = MarksPush {
        head,
        shapes: geometry.address(),
        inks: inks.address(),
        tiles: tiles.address(),
        entries: entries.address(),
        across: shapes.across,
        light: packed(reveal.light),
        dark: packed(reveal.dark),
        square: reveal.square.max(1.0),
        anchor: [reveal.origin.x, reveal.origin.y],
    };
    let io = io
        .read_buffer(&geometry)
        .read_buffer(&inks)
        .read_buffer(&tiles)
        .read_buffer(&entries);
    let [width, height] = extent;
    let groups = [width.div_ceil(TILE), height.div_ceil(TILE), 1];
    let cost = f64::from(width)
        * f64::from(height)
        * (1.0 + shapes.entries.len() as f64 / f64::from(groups[0] * groups[1]));
    work.groups(Shader::Marks, push, io, groups, cost)
}

impl Effect for Marks {
    fn apply(&self, work: &mut Work, frame: &Frame) -> Result<Frame> {
        let erasure = match frame.has_alpha {
            true => Erasure::Remove,
            false => Erasure::Paint,
        };
        let shapes = bin_shapes(
            self.marks.iter(),
            1.0,
            work.canvas_or(frame.extent()),
            erasure,
            |color| frame.space.translated(straight_rgba(color)),
        );
        let out = work.plane_like(&frame.plane, frame.depth.working())?;
        let head = PushHeader::new(&frame.plane, &out)
            .mode(OVER)
            .clamped(frame.depth.clamps())
            .window(work.window());
        draw(
            work,
            head,
            Io::new(&frame.plane, &out),
            out.extent(),
            &shapes,
            ERASED_CHECKERBOARD,
        )?;
        Ok(Frame {
            has_alpha: frame.has_alpha || shapes.erases,
            ..frame.with_plane(out)
        })
    }

    fn scaled(&self, factor: f32) -> Marks {
        Marks::new(self.marks.iter().map(|marking| marking.scaled(factor)))
    }

    fn reach(&self, extent: [u32; 2]) -> Reach {
        let shapes = bin_shapes(
            self.marks.iter(),
            1.0,
            extent,
            Erasure::Paint,
            straight_rgba,
        );
        shapes.bounds(extent).map_or(Reach::Nowhere, Reach::bounded)
    }
}

fn straight_rgba(color: Color) -> [f32; 4] {
    [color.r, color.g, color.b, color.a]
}

fn filled_path(path: &Path, rule: FillRule) -> Marking {
    Marking::Path(PathMark::new(path.clone()).filled(Color::WHITE, rule))
}

pub(crate) fn path_reach(path: &Path, rule: FillRule, scale: f32, extent: [u32; 2]) -> Reach {
    let filled = filled_path(path, rule);
    let shapes = bin_shapes([&filled], scale, extent, Erasure::Paint, straight_rgba);
    shapes.bounds(extent).map_or(Reach::Nowhere, Reach::bounded)
}

pub(crate) fn rasterize_path(
    work: &mut Work,
    path: &Path,
    rule: FillRule,
    scale: f32,
    out: &Plane,
) -> Result<()> {
    let filled = filled_path(path, rule);
    let shapes = bin_shapes(
        [&filled],
        scale,
        work.canvas_or(out.extent()),
        Erasure::Paint,
        straight_rgba,
    );
    let head = PushHeader::new(out, out).mode(LAYER).window(work.window());
    draw(
        work,
        head,
        Io::output(out),
        out.extent(),
        &shapes,
        ERASED_CHECKERBOARD,
    )
}

impl Imaging {
    pub fn marks_layer(
        &self,
        marks: &[Mark],
        live: Option<&Mark>,
        view: View,
    ) -> Result<ComputedFrame> {
        let all: Vec<Marking> = marks
            .iter()
            .chain(live)
            .cloned()
            .map(Marking::from)
            .collect();
        self.layer(&all, view, Erasing::Background)
    }

    pub fn marks_overlay(
        &self,
        marks: &Marks,
        view: View,
        erasing: Erasing,
    ) -> Result<ComputedFrame> {
        self.layer(marks.marks(), view, erasing)
    }

    fn layer(&self, markings: &[Marking], view: View, erasing: Erasing) -> Result<ComputedFrame> {
        let extent = [view.width.max(1), view.height.max(1)];
        let mut work = Work::new(self);
        let layer = work.plane(extent, ImageFormat::Rgba8)?;
        let (erasure, reveal) = match erasing {
            Erasing::Background => (Erasure::Paint, ERASED_CHECKERBOARD),
            Erasing::Checkerboard(checkerboard) => (Erasure::Remove, checkerboard),
        };
        let shapes = bin_shapes(markings, view.scale, extent, erasure, straight_rgba);
        let head = PushHeader::new(&layer, &layer).mode(LAYER);
        draw(&mut work, head, Io::output(&layer), extent, &shapes, reveal)?;
        let ticket = work.finish()?;
        shown_parts(&layer, &ticket)
    }
}
