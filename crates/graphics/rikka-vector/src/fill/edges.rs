use crate::lanes::{LANES, slots};
use rikka_simd::{Float, Simd, Sse2, Vector};

type Single = <Sse2 as Simd>::F32x1;

#[derive(Debug, Default)]
pub(crate) struct Edges {
    len: usize,
    pub(crate) top_x: Vec<f32>,
    pub(crate) top_y: Vec<f32>,
    pub(crate) bottom_x: Vec<f32>,
    pub(crate) bottom_y: Vec<f32>,
    pub(crate) slope: Vec<f32>,
    pub(crate) sign: Vec<f32>,
    pub(crate) first_row: Vec<i32>,
    pub(crate) rows: Vec<i32>,
}

impl Edges {
    #[inline(always)]
    pub(crate) fn clear(&mut self) {
        self.len = 0;
    }

    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub(crate) fn connect<V: Float<Elem = f32>>(
        &mut self,
        xs: &[f32],
        ys: &[f32],
        count: usize,
        dx: f32,
        dy: f32,
    ) {
        let capacity = count.next_multiple_of(LANES);
        if xs.len().min(ys.len()) <= capacity {
            return;
        }
        self.len = count;
        let mut columns = self.columns(capacity);
        let (dx, dy) = (V::splat(dx), V::splat(dy));
        for at in (0..capacity).step_by(V::LANES) {
            let from_x = V::load(&xs[at..]) + dx;
            let from_y = V::load(&ys[at..]) + dy;
            let to_x = V::load(&xs[at + 1..]) + dx;
            let to_y = V::load(&ys[at + 1..]) + dy;
            columns.write::<V>(at, from_x, from_y, to_x, to_y);
        }
    }

    #[inline(always)]
    pub(crate) fn separate(&mut self, ends: &[usize]) {
        for &end in ends {
            if let Some(rows) = self.rows[..self.len].get_mut(end.wrapping_sub(1)) {
                *rows = 0;
            }
        }
    }

    pub(crate) fn push(&mut self, from: (f32, f32), to: (f32, f32)) {
        let at = self.len;
        self.len += 1;
        if self.rows.len() <= at {
            self.columns(self.len.next_multiple_of(LANES));
        }
        let mut columns = Columns {
            top_x: &mut self.top_x,
            top_y: &mut self.top_y,
            bottom_x: &mut self.bottom_x,
            bottom_y: &mut self.bottom_y,
            slope: &mut self.slope,
            sign: &mut self.sign,
            first_row: &mut self.first_row,
            rows: &mut self.rows,
        };
        columns.write::<Single>(
            at,
            Single::splat(from.0),
            Single::splat(from.1),
            Single::splat(to.0),
            Single::splat(to.1),
        );
    }

    #[inline(always)]
    pub(crate) fn span_count(&self) -> usize {
        self.rows[..self.len]
            .iter()
            .fold(0usize, |total, &rows| total + rows as usize)
    }

    #[inline(always)]
    fn columns(&mut self, capacity: usize) -> Columns<'_> {
        Columns {
            top_x: slots(&mut self.top_x, capacity),
            top_y: slots(&mut self.top_y, capacity),
            bottom_x: slots(&mut self.bottom_x, capacity),
            bottom_y: slots(&mut self.bottom_y, capacity),
            slope: slots(&mut self.slope, capacity),
            sign: slots(&mut self.sign, capacity),
            first_row: slots(&mut self.first_row, capacity),
            rows: slots(&mut self.rows, capacity),
        }
    }
}

struct Columns<'c> {
    top_x: &'c mut [f32],
    top_y: &'c mut [f32],
    bottom_x: &'c mut [f32],
    bottom_y: &'c mut [f32],
    slope: &'c mut [f32],
    sign: &'c mut [f32],
    first_row: &'c mut [i32],
    rows: &'c mut [i32],
}

impl Columns<'_> {
    #[inline(always)]
    fn write<V: Float<Elem = f32>>(&mut self, at: usize, x0: V, y0: V, x1: V, y1: V) {
        let down = y0.simd_lt(y1);
        let top_x = V::select(down, x0, x1);
        let top_y = V::select(down, y0, y1);
        let bottom_x = V::select(down, x1, x0);
        let bottom_y = V::select(down, y1, y0);
        let sign = V::select(down, V::splat(1.0), V::splat(-1.0));
        let run = bottom_x - top_x;
        let fall = bottom_y - top_y;
        let first_row = top_y.to_int_trunc();
        let floor_bottom = bottom_y.to_int_trunc();
        let rounds_up = V::from_int(floor_bottom).simd_lt(bottom_y);
        let end_row = floor_bottom + V::Int::select(rounds_up, V::Int::splat(1), V::Int::default());
        let rows = V::Int::select(
            V::default().simd_lt(fall),
            end_row - first_row,
            V::Int::default(),
        );
        top_x.store(&mut self.top_x[at..]);
        top_y.store(&mut self.top_y[at..]);
        bottom_x.store(&mut self.bottom_x[at..]);
        bottom_y.store(&mut self.bottom_y[at..]);
        (run / fall).store(&mut self.slope[at..]);
        sign.store(&mut self.sign[at..]);
        first_row.store(&mut self.first_row[at..]);
        rows.store(&mut self.rows[at..]);
    }
}
