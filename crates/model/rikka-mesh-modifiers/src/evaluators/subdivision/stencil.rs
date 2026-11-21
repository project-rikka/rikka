use std::ops::Range;

use glam::{Vec2, Vec3};

use crate::PARALLEL;

const GRAIN: usize = 1 << 13;

pub(crate) trait Blendable:
    Copy + Send + Sync + std::ops::Mul<f32, Output = Self> + std::ops::Add<Output = Self>
{
    const ZERO: Self;
}

impl Blendable for Vec2 {
    const ZERO: Self = Vec2::ZERO;
}

impl Blendable for Vec3 {
    const ZERO: Self = Vec3::ZERO;
}

struct Chunk {
    rows: Range<usize>,
    offsets: Vec<u32>,
    entries: Vec<(u32, f32)>,
}

pub(crate) struct Stencil {
    chunks: Vec<Chunk>,
    rows: usize,
}

impl Stencil {
    pub(crate) fn build(rows: usize, row: impl Fn(usize, &mut Vec<(u32, f32)>) + Sync) -> Stencil {
        let chunks = PARALLEL.grain(GRAIN).map_chunks(rows, |range| {
            let mut offsets = Vec::with_capacity(range.len() + 1);
            let mut entries = Vec::with_capacity(range.len() * 4);
            offsets.push(0);
            for index in range.clone() {
                row(index, &mut entries);
                offsets.push(entries.len() as u32);
            }
            Chunk {
                rows: range,
                offsets,
                entries,
            }
        });
        Stencil { chunks, rows }
    }

    pub(crate) fn rows(&self) -> usize {
        self.rows
    }

    pub(crate) fn apply<T: Blendable>(&self, values: &[T]) -> Vec<T> {
        self.apply_rows(self.rows, values)
    }

    pub(crate) fn apply_rows<T: Blendable>(&self, rows: usize, values: &[T]) -> Vec<T> {
        let mut output = vec![T::ZERO; rows];
        self.fill_rows(&mut output, |row| {
            row.iter().fold(T::ZERO, |sum, &(point, weight)| {
                sum + values[point as usize] * weight
            })
        });
        output
    }

    pub(crate) fn fill_rows<T: Send>(
        &self,
        output: &mut [T],
        work: impl Fn(&[(u32, f32)]) -> T + Sync,
    ) {
        let rows = output.len().min(self.rows);
        let ranges: Vec<Range<usize>> = self
            .chunks
            .iter()
            .map(|chunk| chunk.rows.start.min(rows)..chunk.rows.end.min(rows))
            .collect();
        let chunks = &self.chunks;
        PARALLEL.split_mut(&mut output[..rows], &ranges, |index, piece| {
            let chunk = &chunks[index];
            for (local, slot) in piece.iter_mut().enumerate() {
                let start = chunk.offsets[local] as usize;
                let end = chunk.offsets[local + 1] as usize;
                *slot = work(&chunk.entries[start..end]);
            }
        });
    }
}
