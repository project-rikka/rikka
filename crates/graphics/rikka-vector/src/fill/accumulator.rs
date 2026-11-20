use rikka_simd::{Float, Simd};

use crate::fill::clip::clip_line;
use crate::fill::edges::Edges;
use crate::fill::resolve::{Grid, resolve_coverage, resolve_mask};
use crate::fill::rule::FillRule;
use crate::fill::spans::{SPAN_REACH, Spans};
use crate::flatten::polylines::Polylines;
use crate::lanes::LANES;

const MAX_SPANS: usize = 1 << 20;

#[derive(Debug, Default)]
pub(crate) struct Accumulator {
    cells: Vec<f32>,
    width: usize,
    height: usize,
    stride: usize,
    edges: Edges,
    spans: Spans,
}

impl Accumulator {
    #[inline(always)]
    pub(crate) fn begin(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.stride = (width + SPAN_REACH).next_multiple_of(LANES);
        let len = self.stride * height;
        if self.cells.len() < len {
            self.cells.resize(len, 0.0);
        }
        self.edges.clear();
    }

    #[inline(always)]
    pub(crate) fn polylines<V: Float<Elem = f32>>(&mut self, lines: &Polylines, dx: f32, dy: f32) {
        self.edges
            .connect::<V>(&lines.xs, &lines.ys, lines.len(), dx, dy);
        self.edges.separate(&lines.ends);
    }

    pub(crate) fn clipped_contour(&mut self, xs: &[f32], ys: &[f32], dx: f32, dy: f32) {
        let (width, height) = (self.width as f32, self.height as f32);
        for (x, y) in xs.windows(2).zip(ys.windows(2)) {
            clip_line(
                &mut self.edges,
                (x[0] + dx, y[0] + dy),
                (x[1] + dx, y[1] + dy),
                width,
                height,
            );
        }
    }

    #[inline(always)]
    pub(crate) fn accumulate<V: Float<Elem = f32>>(&mut self) -> bool {
        if self.edges.span_count() > MAX_SPANS {
            return false;
        }
        let len = self.stride * self.height;
        let cells = &mut self.cells[..len];
        self.spans.walk(&self.edges, self.stride);
        self.spans.shade::<V>();
        self.spans.deposit(cells);
        true
    }

    #[inline(always)]
    pub(crate) fn resolve_mask<S: Simd>(&mut self, out: &mut [u8]) {
        let grid = self.grid();
        resolve_mask::<S>(&mut self.cells[..grid.stride * grid.height], grid, out);
    }

    pub(crate) fn resolve_coverage(&mut self, rule: FillRule, out: &mut [f32]) {
        let grid = self.grid();
        resolve_coverage(&mut self.cells, grid, rule, out);
    }

    #[inline]
    fn grid(&self) -> Grid {
        Grid {
            stride: self.stride,
            width: self.width,
            height: self.height,
        }
    }
}
