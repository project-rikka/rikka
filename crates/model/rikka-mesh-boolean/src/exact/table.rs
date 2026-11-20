use glam::DVec3;
use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::exact::point::Point;
use crate::exact::space::Space;

const MAX_CELL_SPAN: i64 = 4;

#[derive(Debug)]
pub(crate) struct VertexTable {
    space: Space,
    points: Vec<Point>,
    input_count: usize,
    cell: f64,
    grid: FxHashMap<[i64; 3], SmallVec<[u32; 2]>>,
    wide: Vec<u32>,
}

impl VertexTable {
    pub(crate) fn new(space: Space, inputs: Vec<Point>, extent: f64) -> VertexTable {
        let cell = match extent > 0.0 && extent.is_finite() {
            true => extent / 1024.0,
            false => 1.0,
        };
        let mut table = VertexTable {
            space,
            points: Vec::with_capacity(inputs.len()),
            input_count: inputs.len(),
            cell,
            grid: FxHashMap::default(),
            wide: Vec::new(),
        };
        for point in inputs {
            table.push(point);
        }
        table
    }

    pub(crate) fn space(&self) -> &Space {
        &self.space
    }

    pub(crate) fn len(&self) -> usize {
        self.points.len()
    }

    pub(crate) fn point(&self, index: u32) -> &Point {
        &self.points[index as usize]
    }

    pub(crate) fn is_input(&self, index: u32) -> bool {
        (index as usize) < self.input_count
    }

    fn key(&self, position: DVec3) -> [i64; 3] {
        let scaled = position / self.cell;
        [scaled.x, scaled.y, scaled.z].map(|value| value.floor().clamp(-1.0e15, 1.0e15) as i64)
    }

    fn span(&self, point: &Point) -> Option<([i64; 3], [i64; 3])> {
        let low = self.key(point.approx - DVec3::splat(point.error));
        let high = self.key(point.approx + DVec3::splat(point.error));
        let fits = (0..3).all(|axis| high[axis] - low[axis] < MAX_CELL_SPAN);
        match point.approx.is_finite() && fits {
            true => Some((low, high)),
            false => None,
        }
    }

    pub(crate) fn find(&self, point: &Point) -> Option<u32> {
        let candidates: Vec<u32> = match self.span(point) {
            Some((low, high)) => {
                let mut found = Vec::new();
                for x in low[0]..=high[0] {
                    for y in low[1]..=high[1] {
                        for z in low[2]..=high[2] {
                            if let Some(list) = self.grid.get(&[x, y, z]) {
                                found.extend_from_slice(list);
                            }
                        }
                    }
                }
                found.extend_from_slice(&self.wide);
                found
            }
            None => (0..self.points.len() as u32).collect(),
        };
        let mut best: Option<u32> = None;
        for candidate in candidates {
            if best.is_some_and(|best| best <= candidate) {
                continue;
            }
            if self.space.equal(&self.points[candidate as usize], point) {
                best = Some(candidate);
            }
        }
        best
    }

    fn push(&mut self, point: Point) -> u32 {
        let index = self.points.len() as u32;
        match self.span(&point) {
            Some((low, high)) => {
                for x in low[0]..=high[0] {
                    for y in low[1]..=high[1] {
                        for z in low[2]..=high[2] {
                            self.grid.entry([x, y, z]).or_default().push(index);
                        }
                    }
                }
            }
            None => self.wide.push(index),
        }
        self.points.push(point);
        index
    }

    pub(crate) fn find_or_insert(&mut self, point: Point) -> u32 {
        match self.find(&point) {
            Some(index) => index,
            None => self.push(point),
        }
    }
}
