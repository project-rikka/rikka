mod cells;
mod intersect;
mod kernel;
mod polygons;
mod rational;
mod raycast;
mod topology;
mod triangulate;

use glam::{DVec3, Vec3};
use rikka_parallel::Threads;
use rustc_hash::FxHashMap;

use crate::exact::{Point, Space, VertexTable};
use crate::operation::Operation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Knife,
    Boolean(Operation),
}

#[derive(Clone, Debug)]
pub(crate) struct InputFace {
    pub(crate) verts: Vec<u32>,
    pub(crate) edges: Vec<Option<u32>>,
    pub(crate) shape: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct ArrangementInput {
    pub(crate) points: Vec<Vec3>,
    pub(crate) faces: Vec<InputFace>,
    pub(crate) shapes: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Settings {
    pub(crate) mode: Mode,
    pub(crate) use_self: bool,
    pub(crate) hole_tolerant: bool,
    pub(crate) threads: Threads,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OutputVertex {
    pub(crate) origin: Option<u32>,
    pub(crate) position: Vec3,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OutputFace {
    pub(crate) face: u32,
    pub(crate) verts: Vec<u32>,
    pub(crate) edges: Vec<Option<u32>>,
    pub(crate) intersect: Vec<bool>,
    pub(crate) flipped: bool,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Arrangement {
    pub(crate) verts: Vec<OutputVertex>,
    pub(crate) faces: Vec<OutputFace>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Triangle {
    pub(crate) verts: [u32; 3],
    pub(crate) face: u32,
    pub(crate) edges: [Option<u32>; 3],
    pub(crate) intersect: [bool; 3],
    pub(crate) flipped: bool,
}

impl Triangle {
    pub(crate) fn flip(&self) -> Triangle {
        let [a, b, c] = self.verts;
        let [e0, e1, e2] = self.edges;
        let [i0, i1, i2] = self.intersect;
        Triangle {
            verts: [a, c, b],
            face: self.face,
            edges: [e2, e1, e0],
            intersect: [i2, i1, i0],
            flipped: !self.flipped,
        }
    }
}

pub(crate) struct Context<'a> {
    pub(crate) table: VertexTable,
    pub(crate) faces: &'a [InputFace],
    pub(crate) face_verts: Vec<Vec<u32>>,
    pub(crate) shapes: usize,
    pub(crate) settings: Settings,
}

impl Context<'_> {
    pub(crate) fn shape(&self, triangle: &Triangle) -> u32 {
        self.faces[triangle.face as usize].shape
    }

    pub(crate) fn point(&self, vertex: u32) -> &Point {
        self.table.point(vertex)
    }

    pub(crate) fn space(&self) -> &Space {
        self.table.space()
    }
}

fn clean(position: Vec3) -> Vec3 {
    Vec3::from_array(position.to_array().map(|value| match value.is_finite() {
        true => value + 0.0,
        false => 0.0,
    }))
}

pub(crate) fn arrange(input: &ArrangementInput, settings: Settings) -> Arrangement {
    let mut unique: FxHashMap<[u32; 3], u32> = FxHashMap::default();
    let mut points: Vec<Point> = Vec::new();
    let mut origins: Vec<u32> = Vec::new();
    let mut vertex_of: Vec<u32> = Vec::with_capacity(input.points.len());
    let space = Space::new(
        input
            .points
            .iter()
            .flat_map(|position| clean(*position).to_array())
            .map(f64::from),
        true,
    );
    let mut low = DVec3::splat(f64::INFINITY);
    let mut high = DVec3::splat(f64::NEG_INFINITY);
    for (index, &position) in input.points.iter().enumerate() {
        let position = clean(position);
        let key = position.to_array().map(f32::to_bits);
        let id = *unique.entry(key).or_insert_with(|| {
            points.push(space.input_point(position));
            origins.push(index as u32);
            low = low.min(position.as_dvec3());
            high = high.max(position.as_dvec3());
            (points.len() - 1) as u32
        });
        vertex_of.push(id);
    }
    let extent = (high - low).max_element().max(0.0);
    let table = VertexTable::new(space, points, extent);
    let face_verts: Vec<Vec<u32>> = input
        .faces
        .iter()
        .map(|face| face.verts.iter().map(|&v| vertex_of[v as usize]).collect())
        .collect();
    let mut context = Context {
        table,
        faces: &input.faces,
        face_verts,
        shapes: input.shapes.max(1),
        settings,
    };
    let triangles = triangulate::triangulate_faces(&context);
    let needs_intersection = context.shapes > 1 || settings.use_self;
    let subdivided = match needs_intersection {
        true => intersect::intersect(&mut context, &triangles),
        false => triangles,
    };
    let kept = match settings.mode {
        Mode::Knife => subdivided,
        Mode::Boolean(operation) => cells::classify(&mut context, &subdivided, operation),
    };
    polygons::recover(&context, &kept, &origins)
}
