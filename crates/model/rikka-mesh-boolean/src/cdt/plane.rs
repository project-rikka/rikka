use std::cmp::Ordering;

use glam::{DVec2, DVec3};

use crate::cdt::kernel::Kernel;
use crate::exact::{Point, Space};

const SPATIAL_SORT_MIN: usize = 64;
const HILBERT_SIDE: u64 = 1 << 16;

pub(crate) struct PlaneKernel {
    space: Space,
    points: Vec<Point>,
    inputs: usize,
}

impl PlaneKernel {
    pub(crate) fn new(verts: &[DVec2]) -> PlaneKernel {
        let space = Space::new(verts.iter().flat_map(|vert| [vert.x, vert.y]), false);
        PlaneKernel {
            space,
            points: verts
                .iter()
                .map(|vert| space.input_point_f64(DVec3::new(vert.x, vert.y, 0.0)))
                .collect(),
            inputs: verts.len(),
        }
    }

    pub(crate) fn sort_spatially(&self, order: &mut [u32]) {
        if order.len() < SPATIAL_SORT_MIN {
            return;
        }
        let (low, high) = order.iter().fold(
            (DVec2::splat(f64::INFINITY), DVec2::splat(f64::NEG_INFINITY)),
            |(low, high), &index| {
                let point = self.approx(index);
                (low.min(point), high.max(point))
            },
        );
        let extent = (high - low).max(DVec2::splat(f64::MIN_POSITIVE));
        let code = |index: u32| -> u64 {
            let unit = (self.approx(index) - low) / extent;
            let x = (unit.x.clamp(0.0, 1.0) * 65535.0) as u64;
            let y = (unit.y.clamp(0.0, 1.0) * 65535.0) as u64;
            hilbert(x, y)
        };
        order.sort_by_key(|&index| (code(index), index));
    }
}

fn hilbert(mut x: u64, mut y: u64) -> u64 {
    let mut distance = 0u64;
    let mut size = 1u64 << 15;
    while size > 0 {
        let rx = u64::from(x & size != 0);
        let ry = u64::from(y & size != 0);
        distance += size * size * ((3 * rx) ^ ry);
        if ry == 0 {
            if rx == 1 {
                x = HILBERT_SIDE - 1 - x;
                y = HILBERT_SIDE - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        size /= 2;
    }
    distance
}

impl Kernel for PlaneKernel {
    type Support = (u32, u32);

    fn vertex_count(&self) -> usize {
        self.points.len()
    }

    fn approx(&self, vertex: u32) -> DVec2 {
        self.points[vertex as usize].approx.truncate()
    }

    fn orient(&self, a: u32, b: u32, c: u32) -> i32 {
        let p = &self.points;
        self.space
            .orient2d(2, &p[a as usize], &p[b as usize], &p[c as usize])
    }

    fn in_circle(&self, a: u32, b: u32, c: u32, d: u32) -> i32 {
        let p = &self.points;
        self.space.incircle(
            2,
            &p[a as usize],
            &p[b as usize],
            &p[c as usize],
            &p[d as usize],
        )
    }

    fn compare(&self, a: u32, b: u32) -> Ordering {
        let (pa, pb) = (&self.points[a as usize], &self.points[b as usize]);
        self.space
            .compare(0, pa, pb)
            .then_with(|| self.space.compare(1, pa, pb))
    }

    fn intersection(&mut self, first: &(u32, u32), second: &(u32, u32)) -> u32 {
        debug_assert!((first.0 as usize) < self.inputs && (second.0 as usize) < self.inputs);
        let p = &self.points;
        let point = self.space.ssi(
            2,
            &p[first.0 as usize],
            &p[first.1 as usize],
            &p[second.0 as usize],
            &p[second.1 as usize],
        );
        self.points.push(point);
        (self.points.len() - 1) as u32
    }
}
