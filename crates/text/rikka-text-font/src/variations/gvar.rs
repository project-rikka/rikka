use std::marker::PhantomData;

use crate::parse::bytes::Bytes;
use crate::parse::cursor::Cursor;
use crate::parse::f2dot14::F2Dot14;
use crate::sfnt::font::Font;
use crate::sfnt::glyph_id::GlyphId;
use crate::sfnt::table_id::TableId;
use crate::variations::fixed::Fixed;
use crate::variations::packed::{PackedDeltas, PointNumbers};
use crate::variations::tuple_scalar::TupleScalar;
use rikka_types::Point;

const SHARED_POINT_NUMBERS: u16 = 0x8000;
const TUPLE_COUNT_MASK: u16 = 0x0FFF;
const EMBEDDED_PEAK_TUPLE: u16 = 0x8000;
const INTERMEDIATE_REGION: u16 = 0x4000;
const PRIVATE_POINT_NUMBERS: u16 = 0x2000;
const TUPLE_INDEX_MASK: u16 = 0x0FFF;
const LONG_OFFSETS: u16 = 0x0001;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Gvar<'a> {
    axis_count: usize,
    shared_tuples: &'a [u8],
    offsets: &'a [u8],
    long_offsets: bool,
    glyph_count: u16,
    variation_data: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Tuple<'a, S> {
    pub(crate) scalar: S,
    pub(crate) points: PointNumbers<'a>,
    pub(crate) deltas: Cursor<'a>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Tuples<'a, 'c, S> {
    gvar: Gvar<'a>,
    coords: &'c [F2Dot14],
    headers: Cursor<'a>,
    remaining: u16,
    serialized: &'a [u8],
    serialized_offset: usize,
    shared_points: Option<PointNumbers<'a>>,
    scalar: PhantomData<S>,
}

impl<'a> Gvar<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        if data.u16_at(0)? != 1 {
            return None;
        }
        let axis_count = usize::from(data.u16_at(4)?);
        let shared_count = usize::from(data.u16_at(6)?);
        let shared_tuples = data
            .offset32(0, 8)
            .and_then(|tuples| tuples.range(0, shared_count * axis_count * 2));
        let glyph_count = data.u16_at(12)?;
        let long_offsets = data.u16_at(14)? & LONG_OFFSETS != 0;
        let offset_size = if long_offsets { 4 } else { 2 };
        let offsets = data.range(20, (usize::from(glyph_count) + 1) * offset_size)?;
        let variation_data = data.tail(data.u32_at(16)? as usize)?;
        Some(Self {
            axis_count,
            shared_tuples: shared_tuples.unwrap_or_default(),
            offsets,
            long_offsets,
            glyph_count,
            variation_data,
        })
    }

    #[inline]
    fn offset(&self, index: usize) -> Option<usize> {
        if self.long_offsets {
            self.offsets.u32_at(index * 4).map(|offset| offset as usize)
        } else {
            self.offsets
                .u16_at(index * 2)
                .map(|offset| usize::from(offset) * 2)
        }
    }

    fn glyph_data(&self, glyph: GlyphId) -> Option<&'a [u8]> {
        if glyph.0 >= self.glyph_count {
            return None;
        }
        let start = self.offset(glyph.index())?;
        let end = self.offset(glyph.index() + 1)?;
        if end <= start {
            return None;
        }
        self.variation_data.range(start, end - start)
    }

    pub(crate) fn tuples<'c>(
        &self,
        glyph: GlyphId,
        coords: &'c [F2Dot14],
    ) -> Option<Tuples<'a, 'c, f32>> {
        self.tuples_of(glyph, coords)
    }

    pub(crate) fn fixed_tuples<'c>(
        &self,
        glyph: GlyphId,
        coords: &'c [F2Dot14],
    ) -> Option<Tuples<'a, 'c, Fixed>> {
        self.tuples_of(glyph, coords)
    }

    fn tuples_of<'c, S: TupleScalar>(
        &self,
        glyph: GlyphId,
        coords: &'c [F2Dot14],
    ) -> Option<Tuples<'a, 'c, S>> {
        if coords.iter().all(|c| c.0 == 0) {
            return None;
        }
        let data = self.glyph_data(glyph)?;
        let count = data.u16_at(0)?;
        let serialized = data.tail(usize::from(data.u16_at(2)?))?;
        let mut serialized_offset = 0;
        let shared_points = if count & SHARED_POINT_NUMBERS != 0 {
            let mut cursor = Cursor::new(serialized);
            let points = PointNumbers::read(&mut cursor)?;
            serialized_offset = cursor.position();
            Some(points)
        } else {
            None
        };
        Some(Tuples {
            gvar: *self,
            coords,
            headers: Cursor::from_position(data, 4),
            remaining: count & TUPLE_COUNT_MASK,
            serialized,
            serialized_offset,
            shared_points,
            scalar: PhantomData,
        })
    }
}

impl<'a, S: TupleScalar> Iterator for Tuples<'a, '_, S> {
    type Item = Tuple<'a, S>;

    fn next(&mut self) -> Option<Tuple<'a, S>> {
        while self.remaining > 0 {
            self.remaining -= 1;
            let size = usize::from(self.headers.u16()?);
            let index = self.headers.u16()?;
            let axes_size = self.gvar.axis_count * 2;
            let peak = if index & EMBEDDED_PEAK_TUPLE != 0 {
                self.headers.take(axes_size)?
            } else {
                let shared = usize::from(index & TUPLE_INDEX_MASK);
                self.gvar
                    .shared_tuples
                    .range(shared * axes_size, axes_size)?
            };
            let intermediate = if index & INTERMEDIATE_REGION != 0 {
                Some((self.headers.take(axes_size)?, self.headers.take(axes_size)?))
            } else {
                None
            };
            let body = self.serialized.range(self.serialized_offset, size)?;
            self.serialized_offset += size;
            let scalar = S::compute(self.gvar.axis_count, peak, intermediate, self.coords)?;
            if scalar.is_zero() {
                continue;
            }
            let mut cursor = Cursor::new(body);
            let points = if index & PRIVATE_POINT_NUMBERS != 0 {
                PointNumbers::read(&mut cursor)?
            } else {
                self.shared_points?
            };
            return Some(Tuple {
                scalar,
                points,
                deltas: cursor,
            });
        }
        None
    }
}

impl Tuple<'_, Fixed> {
    pub(crate) fn add_horizontal_phantoms(&self, phantom_start: usize, phantoms: &mut [Fixed; 2]) {
        let count = self.points.count(phantom_start + 4);
        let x_deltas = PackedDeltas::new(self.deltas);
        let mut y_deltas = x_deltas;
        if y_deltas.skip_deltas(count).is_none() {
            return;
        }
        for ((index, dx), _) in self.points.iter().take(count).zip(x_deltas).zip(y_deltas) {
            if let Some(phantom) = (index as usize)
                .checked_sub(phantom_start)
                .and_then(|slot| phantoms.get_mut(slot))
            {
                *phantom = phantom.add(Fixed::from_i32(dx).mul(self.scalar));
            }
        }
    }
}

impl Tuple<'_, f32> {
    pub(crate) fn add_deltas(
        &self,
        original: &[Point],
        contour_ends: Option<&[u16]>,
        out: &mut [Point],
        tuple_deltas: &mut Vec<Point>,
        touched: &mut Vec<bool>,
    ) -> Option<()> {
        let total = out.len();
        let count = self.points.count(total);
        let x_deltas = PackedDeltas::new(self.deltas);
        let mut y_deltas = x_deltas;
        y_deltas.skip_deltas(count)?;
        if let PointNumbers::All = self.points {
            let scalar = self.scalar;
            let applied = count.min(out.len());
            let applied = y_deltas.each(applied, |at, dy| out[at].y += dy as f32 * scalar);
            let mut x_deltas = x_deltas;
            x_deltas.each(applied, |at, dx| out[at].x += dx as f32 * scalar);
            return Some(());
        }
        tuple_deltas.clear();
        tuple_deltas.resize(total, Point::default());
        touched.clear();
        touched.resize(total, false);
        for ((index, dx), dy) in self.points.iter().take(count).zip(x_deltas).zip(y_deltas) {
            let index = index as usize;
            if let (Some(delta), Some(mark)) = (tuple_deltas.get_mut(index), touched.get_mut(index))
            {
                *delta = Point::new(dx as f32, dy as f32);
                *mark = true;
            }
        }
        if let Some(ends) = contour_ends {
            let mut start = 0;
            for &end in ends {
                let end = usize::from(end);
                if end < start || end >= total {
                    break;
                }
                interpolate_untouched(
                    original.get(start..=end)?,
                    touched.get(start..=end)?,
                    tuple_deltas.get_mut(start..=end)?,
                );
                start = end + 1;
            }
        }
        for (point, delta) in out.iter_mut().zip(tuple_deltas.iter()) {
            point.x += delta.x * self.scalar;
            point.y += delta.y * self.scalar;
        }
        Some(())
    }

    pub(crate) fn phantom_deltas(&self, phantom_start: usize) -> Option<[Point; 4]> {
        let total = phantom_start + 4;
        let count = self.points.count(total);
        let x_deltas = PackedDeltas::new(self.deltas);
        let mut y_deltas = x_deltas;
        y_deltas.skip_deltas(count)?;
        let mut phantoms = [Point::default(); 4];
        for ((index, dx), dy) in self.points.iter().take(count).zip(x_deltas).zip(y_deltas) {
            if let Some(phantom) = (index as usize)
                .checked_sub(phantom_start)
                .and_then(|i| phantoms.get_mut(i))
            {
                *phantom = Point::new(dx as f32 * self.scalar, dy as f32 * self.scalar);
            }
        }
        Some(phantoms)
    }
}

fn interpolate_untouched(original: &[Point], touched: &[bool], deltas: &mut [Point]) {
    let len = original.len().min(touched.len()).min(deltas.len());
    let Some(first) = (0..len).find(|&i| touched[i]) else {
        return;
    };
    let wrap = |index: usize| if index + 1 == len { 0 } else { index + 1 };
    let mut reference = first;
    loop {
        let mut next = wrap(reference);
        while !touched[next] {
            next = wrap(next);
        }
        let mut index = wrap(reference);
        while index != next {
            let interpolated = Point::new(
                infer(
                    original[index].x,
                    original[reference].x,
                    original[next].x,
                    deltas[reference].x,
                    deltas[next].x,
                ),
                infer(
                    original[index].y,
                    original[reference].y,
                    original[next].y,
                    deltas[reference].y,
                    deltas[next].y,
                ),
            );
            deltas[index] = interpolated;
            index = wrap(index);
        }
        if next <= reference || next == first {
            break;
        }
        reference = next;
    }
}

#[inline]
fn infer(coord: f32, a: f32, b: f32, delta_a: f32, delta_b: f32) -> f32 {
    let (low, high, delta_low, delta_high) = if a <= b {
        (a, b, delta_a, delta_b)
    } else {
        (b, a, delta_b, delta_a)
    };
    if low == high {
        return if delta_low == delta_high {
            delta_low
        } else {
            0.0
        };
    }
    if coord <= low {
        delta_low
    } else if coord >= high {
        delta_high
    } else {
        delta_low + (coord - low) * (delta_high - delta_low) / (high - low)
    }
}

impl<'a> Font<'a> {
    pub(crate) fn gvar(&self) -> Option<Gvar<'a>> {
        Gvar::new(self.table_data(TableId::Gvar)?)
    }
}
