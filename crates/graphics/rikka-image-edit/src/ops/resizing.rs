use std::ops::Range;

use crate::error::Result;
use crate::ops::passes::{
    CopyPush, Direction, FROM_LINEAR, Kernel1d, ResampleAxis, TO_LINEAR, linear_part, resample_part,
};
use crate::picture::{Frame, Mode};
use crate::plane::{Plane, saturating_i32};
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

const COPY: u32 = 0;
const GRAY: u32 = 2;

const ALIGNMENT: u32 = 16;

const PIECE_PIXELS: u64 = 1 << 21;

const MARGIN: f64 = 2.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OutputFormat {
    Working,
    Storage,
}

pub(crate) type Area = [Range<u32>; 2];

pub(crate) struct Resizing<'a> {
    frame: &'a Frame,
    kernel: Kernel1d,
    axes: [Axis; 2],
}

#[derive(Clone, Copy, Debug)]
struct Axis {
    from: u32,
    to: u32,
    scale: f32,
    reach: f64,
}

struct Piece {
    out: Area,
    source: Area,
}

struct Planes {
    raw: Plane,
    linear: Plane,
    across: Option<Plane>,
    down: Option<Plane>,
    straight: Plane,
}

pub(crate) fn resized(
    work: &mut Work,
    frame: &Frame,
    extent: [u32; 2],
    kernel: Kernel1d,
) -> Result<Frame> {
    let out = work.plane(extent, frame.depth.working())?;
    let whole = [0..extent[0], 0..extent[1]];
    Resizing::new(frame, extent, kernel).render(work, whole, &out, OutputFormat::Working)?;
    Ok(frame.with_plane(out))
}

impl Axis {
    fn new(from: u32, to: u32, kernel: Kernel1d) -> Axis {
        let scale = to as f32 / from as f32;
        Axis {
            from,
            to,
            scale,
            reach: f64::from(kernel.support()) / f64::from(scale.min(1.0)),
        }
    }

    fn resamples(self) -> bool {
        self.from != self.to
    }

    fn source_range(self, out: &Range<u32>) -> Range<u32> {
        if !self.resamples() {
            return out.clone();
        }
        let scale = f64::from(self.scale);
        let first = (f64::from(out.start) + 0.5) / scale - self.reach - MARGIN;
        let last = (f64::from(out.end) - 0.5) / scale + self.reach + MARGIN;
        span(first.floor(), last.ceil() + 1.0, self.from)
    }

    fn affected_range(self, changed: &Range<u32>) -> Range<u32> {
        if !self.resamples() {
            return changed.clone();
        }
        let scale = f64::from(self.scale);
        let first = (f64::from(changed.start) - self.reach - MARGIN) * scale - 0.5;
        let last = (f64::from(changed.end) + self.reach + MARGIN) * scale;
        span(first.floor(), last.ceil() + 1.0, self.to)
    }

    fn footprint(self, out: u32) -> u64 {
        let source_size = match self.resamples() {
            true => {
                let spread = f64::from(out) / f64::from(self.scale) + 2.0 * (self.reach + MARGIN);
                (spread.ceil() as u64 + 2).min(u64::from(self.from))
            }
            false => u64::from(out),
        };
        source_size.max(u64::from(out))
    }
}

fn span(start: f64, end: f64, size: u32) -> Range<u32> {
    let limit = f64::from(size);
    start.clamp(0.0, limit) as u32..end.clamp(0.0, limit) as u32
}

fn aligned(span: Range<u32>, size: u32) -> Range<u32> {
    span.start / ALIGNMENT * ALIGNMENT..span.end.next_multiple_of(ALIGNMENT).min(size)
}

fn length(span: &Range<u32>) -> u32 {
    span.end - span.start
}

impl<'a> Resizing<'a> {
    pub(crate) fn new(frame: &'a Frame, extent: [u32; 2], kernel: Kernel1d) -> Resizing<'a> {
        let [width, height] = frame.extent();
        Resizing {
            frame,
            kernel,
            axes: [
                Axis::new(width, extent[0], kernel),
                Axis::new(height, extent[1], kernel),
            ],
        }
    }

    pub(crate) fn affected_range(&self, changed: [u32; 4]) -> Option<Area> {
        let [left, top, width, height] = changed;
        let [across, down] = self.axes;
        let columns = aligned(across.affected_range(&(left..left + width)), across.to);
        let rows = aligned(down.affected_range(&(top..top + height)), down.to);
        (width > 0 && height > 0 && !columns.is_empty() && !rows.is_empty())
            .then_some([columns, rows])
    }

    pub(crate) fn render(
        &self,
        work: &mut Work,
        area: Area,
        into: &Plane,
        output_format: OutputFormat,
    ) -> Result<()> {
        let pieces = self.pieces(area);
        let planes = self.planes(work, &pieces)?;
        for piece in &pieces {
            self.render_piece(work, piece, &planes, into, output_format)?;
        }
        Ok(())
    }

    fn pieces(&self, area: Area) -> Vec<Piece> {
        let [across, down] = self.axes;
        let cost = |columns: u32, rows: u32| across.footprint(columns) * down.footprint(rows);
        let mut columns = length(&area[0]).next_multiple_of(ALIGNMENT);
        while columns > ALIGNMENT && cost(columns, ALIGNMENT) > PIECE_PIXELS {
            columns = (columns / 2).next_multiple_of(ALIGNMENT);
        }
        let tallest = length(&area[1]).next_multiple_of(ALIGNMENT);
        let mut rows = ALIGNMENT;
        while rows < tallest && cost(columns, rows * 2) <= PIECE_PIXELS {
            rows *= 2;
        }
        while rows < tallest && cost(columns, rows + ALIGNMENT) <= PIECE_PIXELS {
            rows += ALIGNMENT;
        }
        let rows = rows.min(tallest);
        let mut pieces = Vec::new();
        for top in area[1].clone().step_by(rows as usize) {
            let out_rows = top..(top + rows).min(area[1].end);
            for left in area[0].clone().step_by(columns as usize) {
                let out_columns = left..(left + columns).min(area[0].end);
                pieces.push(Piece {
                    source: [
                        across.source_range(&out_columns),
                        down.source_range(&out_rows),
                    ],
                    out: [out_columns, out_rows.clone()],
                });
            }
        }
        pieces
    }

    fn planes(&self, work: &mut Work, pieces: &[Piece]) -> Result<Planes> {
        let max_length = |of: fn(&Piece) -> &Range<u32>| {
            pieces
                .iter()
                .map(|piece| length(of(piece)))
                .max()
                .unwrap_or(1)
                .max(1)
        };
        let source_size = [
            max_length(|piece| &piece.source[0]),
            max_length(|piece| &piece.source[1]),
        ];
        let out_size = [
            max_length(|piece| &piece.out[0]),
            max_length(|piece| &piece.out[1]),
        ];
        let working = self.frame.depth.working();
        let [across, down] = self.axes;
        Ok(Planes {
            raw: work.plane(source_size, self.frame.plane.format())?,
            linear: work.plane(source_size, working)?,
            across: across
                .resamples()
                .then(|| work.plane([out_size[0], source_size[1]], working))
                .transpose()?,
            down: down
                .resamples()
                .then(|| work.plane(out_size, working))
                .transpose()?,
            straight: work.plane(out_size, working)?,
        })
    }

    fn render_piece(
        &self,
        work: &mut Work,
        piece: &Piece,
        planes: &Planes,
        into: &Plane,
        output_format: OutputFormat,
    ) -> Result<()> {
        let frame = self.frame;
        let [across, down] = self.axes;
        let source_size = [length(&piece.source[0]), length(&piece.source[1])];
        let out_size = [length(&piece.out[0]), length(&piece.out[1])];
        let copied = CopyPush {
            head: PushHeader::new(&frame.plane, &planes.raw).mode(COPY),
            from: [
                saturating_i32(piece.source[0].start),
                saturating_i32(piece.source[1].start),
            ],
            quarter: 0,
            mirror: 0,
            fill: [0.0; 4],
        };
        let io = Io::new(&frame.plane, &planes.raw);
        work.pass_over(Shader::Copy, copied, io, source_size, 1.0)?;
        let linear = [&planes.raw, &planes.linear];
        linear_part(work, frame, linear, TO_LINEAR, false, source_size)?;
        let horizontal = match &planes.across {
            Some(plane) => {
                let axis = ResampleAxis {
                    direction: Direction::Horizontal,
                    scale: across.scale,
                    start: piece.out[0].start,
                    offset: piece.source[0].start,
                    limit: across.from,
                };
                let extent = [out_size[0], source_size[1]];
                resample_part(work, [&planes.linear, plane], axis, self.kernel, extent)?;
                plane
            }
            None => &planes.linear,
        };
        let vertical_pass = match &planes.down {
            Some(plane) => {
                let axis = ResampleAxis {
                    direction: Direction::Vertical,
                    scale: down.scale,
                    start: piece.out[1].start,
                    offset: piece.source[1].start,
                    limit: down.from,
                };
                resample_part(work, [horizontal, plane], axis, self.kernel, out_size)?;
                plane
            }
            None => horizontal,
        };
        let straight = [vertical_pass, &planes.straight];
        linear_part(
            work,
            frame,
            straight,
            FROM_LINEAR,
            frame.depth.clamps(),
            out_size,
        )?;
        let (mode, clamp) = match (output_format, frame.mode) {
            (OutputFormat::Working, _) => (COPY, false),
            (OutputFormat::Storage, Mode::Gray) => (GRAY, frame.depth.clamps()),
            (OutputFormat::Storage, Mode::Rgb) => (COPY, frame.depth.clamps()),
        };
        let origin = [
            saturating_i32(piece.out[0].start),
            saturating_i32(piece.out[1].start),
        ];
        let store = CopyPush {
            head: PushHeader::new(&planes.straight, into)
                .mode(mode)
                .clamped(clamp)
                .with_origin(origin),
            from: origin.map(|value| -value),
            quarter: 0,
            mirror: 0,
            fill: [0.0; 4],
        };
        let groups = [out_size[0].div_ceil(16), out_size[1].div_ceil(16), 1];
        let cost = f64::from(out_size[0]) * f64::from(out_size[1]);
        let out_area = [
            piece.out[0].start,
            piece.out[1].start,
            out_size[0],
            out_size[1],
        ];
        let io = Io::new(&planes.straight, into).limit_writes(out_area);
        work.groups(Shader::Copy, store, io, groups, cost)
    }
}
