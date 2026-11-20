use crate::fill::edges::Edges;
use crate::lanes::{LANES, slots};
use rikka_simd::{Float, Mask, ScalarFloat, Vector};

pub(crate) const SPAN_REACH: usize = 3;

#[derive(Debug, Default)]
pub(crate) struct Spans {
    len: usize,
    base: Vec<i32>,
    xa: Vec<f32>,
    xb: Vec<f32>,
    height: Vec<f32>,
    target: Vec<i32>,
    head: Vec<f32>,
    middle: Vec<f32>,
    tail: Vec<f32>,
    wide: Vec<usize>,
}

impl Spans {
    #[inline(always)]
    pub(crate) fn walk(&mut self, edges: &Edges, stride: usize) {
        let count = edges.len();
        self.len = edges.span_count();
        self.reserve(self.len.next_multiple_of(LANES));
        let stride = stride as i32;
        let top_x = &edges.top_x[..count];
        let top_y = &edges.top_y[..count];
        let bottom_x = &edges.bottom_x[..count];
        let bottom_y = &edges.bottom_y[..count];
        let slope = &edges.slope[..count];
        let sign = &edges.sign[..count];
        let first_row = &edges.first_row[..count];
        let rows = &edges.rows[..count];
        let capacity = self.len.next_multiple_of(LANES);
        let base_out = &mut self.base[..capacity];
        let xa_out = &mut self.xa[..capacity];
        let xb_out = &mut self.xb[..capacity];
        let height_out = &mut self.height[..capacity];
        let mut put = |at: usize, base: i32, xa: f32, xb: f32, height: f32| {
            base_out[at] = base;
            xa_out[at] = xa;
            xb_out[at] = xb;
            height_out[at] = height;
        };
        let mut cursor = 0;
        for index in 0..count {
            let span_rows = rows[index];
            if span_rows == 0 {
                continue;
            }
            let (start, top, bottom) = (top_x[index], top_y[index], bottom_y[index]);
            let (slope, sign) = (slope[index], sign[index]);
            let row = first_row[index];
            let mut base = row * stride;
            let (mut x, mut y) = (start, top);
            let mut next_y = (row + 1) as f32;
            let end = cursor + span_rows as usize - 1;
            while cursor < end {
                let next_x = start + (next_y - top) * slope;
                put(cursor, base, x, next_x, (next_y - y) * sign);
                x = next_x;
                y = next_y;
                next_y += 1.0;
                base += stride;
                cursor += 1;
            }
            let height = (bottom - y) * sign;
            put(cursor, base, x, bottom_x[index], height);
            cursor += 1;
        }
    }

    #[inline(always)]
    pub(crate) fn shade<V: Float<Elem = f32>>(&mut self) {
        let (zero, one, half) = (V::splat(0.0), V::splat(1.0), V::splat(0.5));
        let len = self.len;
        let capacity = len.next_multiple_of(LANES);
        self.wide.clear();
        let (base, xa, xb) = (
            &self.base[..capacity],
            &self.xa[..capacity],
            &self.xb[..capacity],
        );
        let heights = &self.height[..capacity];
        let targets = &mut self.target[..capacity];
        let heads = &mut self.head[..capacity];
        let middles = &mut self.middle[..capacity];
        let tails = &mut self.tail[..capacity];
        for at in (0..capacity).step_by(V::LANES) {
            let a = V::load(&xa[at..]);
            let b = V::load(&xb[at..]);
            let x0 = a.min(b);
            let x1 = a.max(b);
            let cell = x0.to_int_trunc();
            let left = V::from_int(cell);
            let boundary = left + one;
            let crosses = boundary.simd_lt(x1);
            let split = V::select(crosses, boundary, x1);
            let height = V::load(&heights[at..]);
            let head = V::select(crosses, height * (boundary - x0) / (x1 - x0), height);
            let tail = height - head;
            let head_share = head * ((x0 + split) * half - left);
            let tail_share = tail * ((split + x1) * half - boundary);
            let wide = (boundary + one).simd_lt(x1);
            let middle = head_share + (tail - tail_share);
            (V::Int::load(&base[at..]) + cell).store(&mut targets[at..]);
            V::select(wide, zero, head - head_share).store(&mut heads[at..]);
            V::select(wide, zero, middle).store(&mut middles[at..]);
            V::select(wide, zero, tail_share).store(&mut tails[at..]);
            let mut lanes = wide.to_bitmask();
            while lanes != 0 {
                let index = at + lanes.trailing_zeros() as usize;
                if index < len {
                    self.wide.push(index);
                }
                lanes &= lanes - 1;
            }
        }
    }

    #[inline(always)]
    pub(crate) fn deposit(&self, cells: &mut [f32]) {
        let count = self.len;
        let target = &self.target[..count];
        let head = &self.head[..count];
        let middle = &self.middle[..count];
        let tail = &self.tail[..count];
        for index in 0..count {
            let start = target[index] as usize;
            let Some(run) = cells.get_mut(start..start.wrapping_add(SPAN_REACH)) else {
                continue;
            };
            run[0] += head[index];
            run[1] += middle[index];
            run[2] += tail[index];
        }
        for &index in &self.wide {
            self.deposit_wide(index, cells);
        }
    }

    #[inline(always)]
    fn reserve(&mut self, capacity: usize) {
        for column in [
            &mut self.xa,
            &mut self.xb,
            &mut self.height,
            &mut self.head,
            &mut self.middle,
            &mut self.tail,
        ] {
            slots(column, capacity);
        }
        slots(&mut self.base, capacity);
        slots(&mut self.target, capacity);
    }

    #[cold]
    #[inline(never)]
    fn deposit_wide(&self, index: usize, cells: &mut [f32]) {
        let (a, b) = (self.xa[index], self.xb[index]);
        let x0 = if a < b { a } else { b };
        let x1 = if a > b { a } else { b };
        let height = self.height[index];
        let cell = x0.to_int_trunc();
        let last = x1.to_int_trunc();
        let interior = (last - cell) as usize;
        let left = cell as f32;
        let start = self.base[index].wrapping_add(cell) as usize;
        let Some(run) = cells.get_mut(start..start.wrapping_add(interior + 2)) else {
            return;
        };
        let ratio = height / (x1 - x0);
        let head = left + 1.0 - x0;
        let mut covered = 0.5 * ratio * head * head;
        run[0] += covered;
        let offset = 0.5 - (x0 - left);
        for (step, target) in run[1..interior].iter_mut().enumerate() {
            let area = ratio * ((step + 1) as f32 + offset);
            *target += area - covered;
            covered = area;
        }
        let tail = (last + 1) as f32 - x1;
        let area = ratio * (interior as f32 + offset - 0.5 * tail * tail);
        run[interior] += area - covered;
        run[interior + 1] += height - area;
    }
}
