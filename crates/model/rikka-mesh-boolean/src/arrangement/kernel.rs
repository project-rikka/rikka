use std::cmp::Ordering;

use glam::DVec2;

use crate::arrangement::Triangle;
use crate::cdt::kernel::Kernel;
use crate::exact::{Point, VertexTable, projection};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Support {
    Edge(u32, u32),
    Plane(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VertexRef {
    Table(u32),
    New(u32),
}

pub(crate) struct FaceKernel<'a> {
    table: &'a VertexTable,
    triangles: &'a [Triangle],
    plane: [u32; 3],
    axis: usize,
    flip: bool,
    pub(crate) vertices: Vec<VertexRef>,
    pub(crate) created: Vec<Point>,
}

impl<'a> FaceKernel<'a> {
    pub(crate) fn new(
        table: &'a VertexTable,
        triangles: &'a [Triangle],
        plane: [u32; 3],
    ) -> FaceKernel<'a> {
        let [a, b, c] = plane.map(|v| table.point(v));
        let normal = (b.approx - a.approx).cross(c.approx - a.approx).abs();
        let mut axes = [0usize, 1, 2];
        axes.sort_by(|&x, &y| normal[y].total_cmp(&normal[x]));
        let space = table.space();
        let (axis, orientation) = axes
            .iter()
            .map(|&axis| (axis, space.orient2d(axis, a, b, c)))
            .find(|&(_, orientation)| orientation != 0)
            .unwrap_or((axes[0], 1));
        FaceKernel {
            table,
            triangles,
            plane,
            axis,
            flip: orientation < 0,
            vertices: Vec::new(),
            created: Vec::new(),
        }
    }

    pub(crate) fn add(&mut self, vertex: VertexRef) -> u32 {
        self.vertices.push(vertex);
        (self.vertices.len() - 1) as u32
    }

    pub(crate) fn point(&self, vertex: u32) -> &Point {
        match self.vertices[vertex as usize] {
            VertexRef::Table(id) => self.table.point(id),
            VertexRef::New(index) => &self.created[index as usize],
        }
    }

    pub(crate) fn is_clockwise(&self, verts: [u32; 3]) -> bool {
        let [a, b, c] = verts.map(|v| self.table.point(v));
        let orientation = self.table.space().orient2d(self.axis, a, b, c);
        (orientation < 0) != self.flip
    }

    fn triangle_points(&self, triangle: u32) -> [&Point; 3] {
        self.triangles[triangle as usize]
            .verts
            .map(|v| self.table.point(v))
    }

    fn sign(&self, value: i32) -> i32 {
        match self.flip {
            true => -value,
            false => value,
        }
    }
}

impl Kernel for FaceKernel<'_> {
    type Support = Support;

    fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    fn approx(&self, vertex: u32) -> DVec2 {
        let (u, v) = projection(self.axis);
        let point = self.point(vertex).approx;
        DVec2::new(point[u], point[v])
    }

    fn orient(&self, a: u32, b: u32, c: u32) -> i32 {
        let value =
            self.table
                .space()
                .orient2d(self.axis, self.point(a), self.point(b), self.point(c));
        self.sign(value)
    }

    fn in_circle(&self, a: u32, b: u32, c: u32, d: u32) -> i32 {
        let value = self.table.space().incircle(
            self.axis,
            self.point(a),
            self.point(b),
            self.point(c),
            self.point(d),
        );
        self.sign(value)
    }

    fn compare(&self, a: u32, b: u32) -> Ordering {
        let (u, v) = projection(self.axis);
        let space = self.table.space();
        let (pa, pb) = (self.point(a), self.point(b));
        space
            .compare(u, pa, pb)
            .then_with(|| space.compare(v, pa, pb))
    }

    fn intersection(&mut self, first: &Support, second: &Support) -> u32 {
        let space = self.table.space();
        let table = self.table;
        let point = match (*first, *second) {
            (Support::Edge(p, q), Support::Edge(r, s)) => space.ssi(
                self.axis,
                table.point(p),
                table.point(q),
                table.point(r),
                table.point(s),
            ),
            (Support::Edge(p, q), Support::Plane(t)) | (Support::Plane(t), Support::Edge(p, q)) => {
                space.lpi(table.point(p), table.point(q), self.triangle_points(t))
            }
            (Support::Plane(t1), Support::Plane(t2)) => space.tpi([
                self.plane.map(|v| table.point(v)),
                self.triangle_points(t1),
                self.triangle_points(t2),
            ]),
        };
        self.created.push(point);
        let index = (self.created.len() - 1) as u32;
        self.add(VertexRef::New(index))
    }
}
