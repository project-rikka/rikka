use std::f32::consts::PI;

use glam::{Mat3, Mat4, Vec2, Vec3};

use crate::error::{Error, Result};
use crate::ids::{FaceId, LoopId, VertId};
use crate::join::DoubleFace;
use crate::mesh::EditMesh;
use crate::sets::IdSet;
use crate::walk::Walk;

const DEFAULT_UV_MAP: &str = "UVMap";
const ICO_SCALE: f32 = 200.0;

const ICO_VERTS: [[f32; 3]; 12] = [
    [0.0, 0.0, -200.0],
    [144.72, -105.144, -89.443],
    [-55.277, -170.128, -89.443],
    [-178.885, 0.0, -89.443],
    [-55.277, 170.128, -89.443],
    [144.72, 105.144, -89.443],
    [55.277, -170.128, 89.443],
    [-144.72, -105.144, 89.443],
    [-144.72, 105.144, 89.443],
    [55.277, 170.128, 89.443],
    [178.885, 0.0, 89.443],
    [0.0, 0.0, 200.0],
];

const ICO_FACES: [[usize; 3]; 20] = [
    [0, 1, 2],
    [1, 0, 5],
    [0, 2, 3],
    [0, 3, 4],
    [0, 4, 5],
    [1, 5, 10],
    [2, 1, 6],
    [3, 2, 7],
    [4, 3, 8],
    [5, 4, 9],
    [1, 10, 6],
    [2, 6, 7],
    [3, 7, 8],
    [4, 8, 9],
    [5, 9, 10],
    [6, 10, 11],
    [7, 6, 11],
    [8, 7, 11],
    [9, 8, 11],
    [10, 9, 11],
];

const ICO_UV_COLUMNS: f32 = 11.0;
const ICO_UV_ROW: f32 = 0.157_461;

const ICO_NET: [[[u8; 2]; 3]; 20] = [
    [[2, 0], [3, 1], [1, 1]],
    [[3, 1], [4, 0], [5, 1]],
    [[10, 0], [11, 1], [9, 1]],
    [[8, 0], [9, 1], [7, 1]],
    [[6, 0], [7, 1], [5, 1]],
    [[3, 1], [5, 1], [4, 2]],
    [[1, 1], [3, 1], [2, 2]],
    [[9, 1], [11, 1], [10, 2]],
    [[7, 1], [9, 1], [8, 2]],
    [[5, 1], [7, 1], [6, 2]],
    [[3, 1], [4, 2], [2, 2]],
    [[1, 1], [2, 2], [0, 2]],
    [[9, 1], [10, 2], [8, 2]],
    [[7, 1], [8, 2], [6, 2]],
    [[5, 1], [6, 2], [4, 2]],
    [[2, 2], [4, 2], [3, 3]],
    [[0, 2], [2, 2], [1, 3]],
    [[8, 2], [10, 2], [9, 3]],
    [[6, 2], [8, 2], [7, 3]],
    [[4, 2], [6, 2], [5, 3]],
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CircleFill {
    #[default]
    Nothing,
    Ngon,
    TriangleFan,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Primitive {
    Plane {
        size: f32,
    },
    Cube {
        size: f32,
    },
    Circle {
        vertices: u32,
        radius: f32,
        fill: CircleFill,
    },
    UvSphere {
        segments: u32,
        rings: u32,
        radius: f32,
    },
    IcoSphere {
        subdivisions: u32,
        radius: f32,
    },
    Cylinder {
        vertices: u32,
        radius: f32,
        depth: f32,
        cap: CircleFill,
    },
    Cone {
        vertices: u32,
        radius_bottom: f32,
        radius_top: f32,
        depth: f32,
        cap: CircleFill,
    },
    Torus {
        major_segments: u32,
        minor_segments: u32,
        major_radius: f32,
        minor_radius: f32,
    },
    Grid {
        x_subdivisions: u32,
        y_subdivisions: u32,
        size: f32,
    },
}

impl Primitive {
    #[must_use]
    pub fn plane() -> Primitive {
        Primitive::Plane { size: 2.0 }
    }

    #[must_use]
    pub fn cube() -> Primitive {
        Primitive::Cube { size: 2.0 }
    }

    #[must_use]
    pub fn circle() -> Primitive {
        Primitive::Circle {
            vertices: 32,
            radius: 1.0,
            fill: CircleFill::Nothing,
        }
    }

    #[must_use]
    pub fn uv_sphere() -> Primitive {
        Primitive::UvSphere {
            segments: 32,
            rings: 16,
            radius: 1.0,
        }
    }

    #[must_use]
    pub fn ico_sphere() -> Primitive {
        Primitive::IcoSphere {
            subdivisions: 2,
            radius: 1.0,
        }
    }

    #[must_use]
    pub fn cylinder() -> Primitive {
        Primitive::Cylinder {
            vertices: 32,
            radius: 1.0,
            depth: 2.0,
            cap: CircleFill::Ngon,
        }
    }

    #[must_use]
    pub fn cone() -> Primitive {
        Primitive::Cone {
            vertices: 32,
            radius_bottom: 1.0,
            radius_top: 0.0,
            depth: 2.0,
            cap: CircleFill::Ngon,
        }
    }

    #[must_use]
    pub fn torus() -> Primitive {
        Primitive::Torus {
            major_segments: 48,
            minor_segments: 12,
            major_radius: 1.0,
            minor_radius: 0.25,
        }
    }

    #[must_use]
    pub fn grid() -> Primitive {
        Primitive::Grid {
            x_subdivisions: 10,
            y_subdivisions: 10,
            size: 2.0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AddedPrimitive {
    pub verts: Vec<VertId>,
    pub faces: Vec<FaceId>,
}

pub(crate) fn sin_cos_from_fraction(numerator: i32, denominator: i32) -> (f32, f32) {
    let mut numerator = numerator * 8;
    let octant = numerator / denominator;
    let denominator = denominator * 8;
    let mut cos_sign = 1.0f32;
    let mut swapped = false;
    match octant {
        0 => {}
        1 | 2 => {
            numerator = denominator / 4 - numerator;
            swapped = true;
        }
        3 | 4 => {
            numerator = denominator / 2 - numerator;
            cos_sign = -1.0;
        }
        5 | 6 => {
            numerator -= denominator * 3 / 4;
            swapped = true;
            cos_sign = -1.0;
        }
        _ => numerator -= denominator,
    }
    let angle = (2.0 * std::f64::consts::PI) as f32 * (numerator as f32 / denominator as f32);
    let (sin, cos) = (angle.sin(), angle.cos() * cos_sign);
    match swapped {
        true => (cos, sin),
        false => (sin, cos),
    }
}

fn check(condition: bool, name: &'static str) -> Result<()> {
    match condition {
        true => Ok(()),
        false => Err(Error::PrimitiveParameter(name)),
    }
}

struct Builder<'a> {
    mesh: &'a mut EditMesh,
    matrix: Mat4,
    uv_map: Option<String>,
    added: AddedPrimitive,
}

impl Builder<'_> {
    fn vert(&mut self, local: Vec3) -> VertId {
        let vert = self.mesh.add_vert(self.matrix.transform_point3(local));
        self.added.verts.push(vert);
        vert
    }

    fn raw_vert(&mut self, position: Vec3) -> VertId {
        let vert = self.mesh.add_vert(position);
        self.added.verts.push(vert);
        vert
    }

    fn face(&mut self, verts: &[VertId]) -> Result<FaceId> {
        let face = self.mesh.add_face(verts)?;
        self.added.faces.push(face);
        Ok(face)
    }

    fn set_uvs(&mut self, face: FaceId, uvs: &[Vec2]) {
        let Some(name) = self.uv_map.clone() else {
            return;
        };
        let loops: Vec<LoopId> = self.mesh.face_loops(face).collect();
        if let Some(values) = self.mesh.loop_uvs_mut(&name) {
            for (lp, &uv) in loops.iter().zip(uvs) {
                values[lp.index()] = uv;
            }
        }
    }

    fn local(&self, point: Vec3) -> Vec3 {
        self.matrix.inverse().transform_point3(point)
    }

    fn finish(mut self) -> AddedPrimitive {
        self.added
            .verts
            .retain(|&vert| self.mesh.contains_vert(vert));
        self.added
            .faces
            .retain(|&face| self.mesh.contains_face(face));
        self.added
    }
}

impl EditMesh {
    pub fn add_primitive(
        &mut self,
        primitive: &Primitive,
        matrix: Mat4,
        calc_uvs: bool,
    ) -> Result<AddedPrimitive> {
        let uv_map = match calc_uvs {
            false => None,
            true => {
                if self.layout.uv_maps.is_empty() {
                    self.add_uv_map(DEFAULT_UV_MAP)?;
                }
                self.layout.uv_maps.active_name().map(str::to_owned)
            }
        };
        let mut builder = Builder {
            mesh: self,
            matrix,
            uv_map,
            added: AddedPrimitive::default(),
        };
        match *primitive {
            Primitive::Plane { size } => builder.grid(1, 1, size / 2.0)?,
            Primitive::Grid {
                x_subdivisions,
                y_subdivisions,
                size,
            } => {
                check(x_subdivisions >= 1, "x_subdivisions")?;
                check(y_subdivisions >= 1, "y_subdivisions")?;
                builder.grid(x_subdivisions, y_subdivisions, size / 2.0)?;
            }
            Primitive::Cube { size } => builder.cube(size)?,
            Primitive::Circle {
                vertices,
                radius,
                fill,
            } => {
                check(vertices >= 3, "vertices")?;
                builder.circle(vertices, radius, fill)?;
            }
            Primitive::Cylinder {
                vertices,
                radius,
                depth,
                cap,
            } => {
                check(vertices >= 3, "vertices")?;
                builder.cone(vertices, radius, radius, depth, cap)?;
            }
            Primitive::Cone {
                vertices,
                radius_bottom,
                radius_top,
                depth,
                cap,
            } => {
                check(vertices >= 3, "vertices")?;
                check(radius_bottom >= 0.0 && radius_top >= 0.0, "radius")?;
                builder.cone(vertices, radius_bottom, radius_top, depth, cap)?;
            }
            Primitive::UvSphere {
                segments,
                rings,
                radius,
            } => {
                check(segments >= 3, "segments")?;
                check(rings >= 3, "rings")?;
                builder.uv_sphere(segments, rings, radius)?;
            }
            Primitive::IcoSphere {
                subdivisions,
                radius,
            } => {
                check((1..=10).contains(&subdivisions), "subdivisions")?;
                builder.ico_sphere(subdivisions, radius)?;
            }
            Primitive::Torus {
                major_segments,
                minor_segments,
                major_radius,
                minor_radius,
            } => {
                check(major_segments >= 3, "major_segments")?;
                check(minor_segments >= 3, "minor_segments")?;
                builder.torus(major_segments, minor_segments, major_radius, minor_radius)?;
            }
        }
        let added = builder.finish();
        self.update_normals_around(added.verts.iter().copied());
        Ok(added)
    }
}

impl Builder<'_> {
    fn grid(&mut self, x_segments: u32, y_segments: u32, half: f32) -> Result<()> {
        let x_step = 2.0 / x_segments as f32;
        let y_step = 2.0 / y_segments as f32;
        let mut verts = Vec::with_capacity(((x_segments + 1) * (y_segments + 1)) as usize);
        for y in 0..=y_segments {
            let local_y = ((y as f32 * y_step) - 1.0) * half;
            for x in 0..=x_segments {
                let local_x = ((x as f32 * x_step) - 1.0) * half;
                verts.push(self.vert(Vec3::new(local_x, local_y, 0.0)));
            }
        }
        let at = |x: u32, y: u32| verts[(x + y * (x_segments + 1)) as usize];
        let (du, dv) = (1.0 / x_segments as f32, 1.0 / y_segments as f32);
        let mut index = 0;
        for y in 1..=y_segments {
            for x in 1..=x_segments {
                let face = self.face(&[at(x - 1, y - 1), at(x, y - 1), at(x, y), at(x - 1, y)])?;
                let (ix, iy) = ((index % x_segments) as f32, (index / x_segments) as f32);
                self.set_uvs(
                    face,
                    &[
                        Vec2::new(ix * du, iy * dv),
                        Vec2::new((ix + 1.0) * du, iy * dv),
                        Vec2::new((ix + 1.0) * du, (iy + 1.0) * dv),
                        Vec2::new(ix * du, (iy + 1.0) * dv),
                    ],
                );
                index += 1;
            }
        }
        Ok(())
    }

    fn cube(&mut self, size: f32) -> Result<()> {
        let mut offset = size / 2.0;
        if offset == 0.0 {
            offset = 0.5;
        }
        let mut verts = Vec::with_capacity(8);
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    verts.push(self.vert(Vec3::new(x, y, z) * offset));
                }
            }
        }
        const FACES: [[usize; 4]; 6] = [
            [0, 1, 3, 2],
            [2, 3, 7, 6],
            [6, 7, 5, 4],
            [4, 5, 1, 0],
            [2, 6, 4, 0],
            [7, 3, 1, 5],
        ];
        let (mut x, mut y) = (0.375f32, 0.0f32);
        const WIDTH: f32 = 0.25;
        for corners in FACES {
            let face = self.face(&corners.map(|corner| verts[corner]))?;
            let mut uvs = [Vec2::ZERO; 4];
            for (index, uv) in uvs.iter_mut().enumerate() {
                *uv = Vec2::new(x, y);
                match index {
                    0 => x += WIDTH,
                    1 => y += WIDTH,
                    2 => x -= WIDTH,
                    _ => y -= WIDTH,
                }
            }
            self.set_uvs(face, &uvs);
            if y >= 0.75 && x > 0.125 {
                x = 0.125;
                y = 0.5;
            } else if x <= 0.125 {
                x = 0.625;
                y = 0.5;
            } else {
                y += 0.25;
            }
        }
        Ok(())
    }

    fn circle_uvs(&mut self, faces: &[FaceId], radius: f32) {
        let scale = 0.5 / radius;
        for &face in faces {
            let uvs: Vec<Vec2> = self
                .mesh
                .face_verts(face)
                .map(|vert| {
                    let local = self.local(self.mesh.position(vert));
                    Vec2::new(0.5 + scale * local.x, 0.5 + scale * local.y)
                })
                .collect();
            self.set_uvs(face, &uvs);
        }
    }

    fn circle(&mut self, segments: u32, radius: f32, fill: CircleFill) -> Result<()> {
        let cap = fill != CircleFill::Nothing;
        let center = cap.then(|| self.vert(Vec3::ZERO));
        let mut first = None;
        let mut last: Option<VertId> = None;
        let mut fan = Vec::new();
        for index in 0..segments as i32 {
            let (sin, cos) = sin_cos_from_fraction(index, segments as i32);
            let vert = self.vert(Vec3::new(sin * -radius, cos * radius, 0.0));
            if let Some(previous) = last {
                self.mesh.add_edge(vert, previous)?;
                if let Some(center) = center {
                    fan.push(self.face(&[center, previous, vert])?);
                }
            }
            first.get_or_insert(vert);
            last = Some(vert);
        }
        let (Some(first), Some(last)) = (first, last) else {
            return Ok(());
        };
        self.mesh.add_edge(first, last)?;
        if let Some(center) = center {
            fan.push(self.face(&[center, last, first])?);
            self.circle_uvs(&fan, radius);
        }
        if fill == CircleFill::Ngon {
            self.dissolve(&fan)?;
        }
        Ok(())
    }

    fn dissolve(&mut self, faces: &[FaceId]) -> Result<()> {
        let mut mask: IdSet<FaceId> = faces
            .iter()
            .copied()
            .filter(|&face| self.mesh.contains_face(face))
            .collect();
        let order: Vec<FaceId> = mask.iter().collect();
        let mut pending = mask.clone();
        let mut regions: Vec<Vec<FaceId>> = Vec::new();
        for face in order {
            if !pending.contains(face) {
                continue;
            }
            let walk = Walk {
                faces: Some(&mask),
                ..Walk::default()
            };
            let region = self.mesh.walk_island(face, &walk, true);
            if region.len() >= 2 {
                for &member in &region {
                    pending.remove(member);
                }
                regions.push(region);
            }
        }
        for region in regions {
            let joined = self.mesh.join_faces(&region, DoubleFace::Keep)?;
            let face = match joined.double {
                Some(double) => {
                    self.mesh.kill_face(joined.face)?;
                    double
                }
                None => joined.face,
            };
            mask.insert(face);
            self.added.faces.push(face);
        }
        Ok(())
    }

    fn cone(
        &mut self,
        segments: u32,
        radius_bottom: f32,
        radius_top: f32,
        depth: f32,
        fill: CircleFill,
    ) -> Result<()> {
        let half = depth * 0.5;
        let cap = fill != CircleFill::Nothing;
        let centers = cap.then(|| {
            (
                self.vert(Vec3::new(0.0, 0.0, -half)),
                self.vert(Vec3::new(0.0, 0.0, half)),
            )
        });
        let mut caps: Vec<FaceId> = Vec::new();
        let mut marked: Vec<FaceId> = Vec::new();
        let mut sides: Vec<FaceId> = Vec::new();
        let mut first: Option<(VertId, VertId)> = None;
        let mut last: Option<(VertId, VertId)> = None;
        for index in 0..segments as i32 {
            let (sin, cos) = sin_cos_from_fraction(index, segments as i32);
            let bottom = self.vert(Vec3::new(radius_bottom * sin, radius_bottom * cos, -half));
            let top = self.vert(Vec3::new(radius_top * sin, radius_top * cos, half));
            match last {
                Some((last_bottom, last_top)) => {
                    if let Some((center_bottom, center_top)) = centers {
                        let face = self.face(&[center_bottom, last_bottom, bottom])?;
                        caps.push(face);
                        marked.push(face);
                        let face = self.face(&[center_top, top, last_top])?;
                        caps.push(face);
                        marked.push(face);
                    }
                    let face = self.face(&[last_bottom, last_top, top, bottom])?;
                    marked.push(face);
                    sides.push(face);
                }
                None => first = Some((bottom, top)),
            }
            last = Some((bottom, top));
        }
        let (Some((first_bottom, first_top)), Some((last_bottom, last_top))) = (first, last) else {
            return Ok(());
        };
        if let Some((center_bottom, center_top)) = centers {
            let face = self.face(&[center_bottom, last_bottom, first_bottom])?;
            caps.push(face);
            marked.push(face);
            let face = self.face(&[center_top, first_top, last_top])?;
            caps.push(face);
            marked.push(face);
        }
        let face = self.face(&[last_bottom, last_top, first_top, first_bottom])?;
        marked.push(face);
        self.cone_uvs(&marked, radius_top, radius_bottom, segments, cap);
        if radius_bottom == 0.0 {
            if let Some((center_bottom, _)) = centers {
                self.mesh.kill_vert(center_bottom)?;
            }
            for &side in &sides {
                if !self.mesh.contains_face(side) {
                    continue;
                }
                let first_loop = self.mesh.face_first_loop(side);
                let previous = self.mesh.loop_prev(first_loop);
                let edge = self.mesh.loop_edge(previous);
                self.mesh
                    .collapse_edge(edge, self.mesh.loop_vert(previous))?;
            }
        }
        if radius_top == 0.0 {
            if let Some((_, center_top)) = centers {
                self.mesh.kill_vert(center_top)?;
            }
            for &side in &sides {
                if !self.mesh.contains_face(side) {
                    continue;
                }
                let first_loop = self.mesh.face_first_loop(side);
                let next = self.mesh.loop_next(first_loop);
                let edge = self.mesh.loop_edge(next);
                self.mesh.collapse_edge(edge, self.mesh.loop_vert(next))?;
            }
        }
        if fill == CircleFill::Ngon {
            let alive: Vec<FaceId> = caps
                .into_iter()
                .filter(|&face| self.mesh.contains_face(face))
                .collect();
            self.dissolve(&alive)?;
        }
        Ok(())
    }

    fn cone_uvs(
        &mut self,
        faces: &[FaceId],
        radius_top: f32,
        radius_bottom: f32,
        segments: u32,
        cap: bool,
    ) {
        if self.uv_map.is_none() {
            return;
        }
        let width = 1.0 / segments as f32;
        let height = if cap { 0.5 } else { 1.0 };
        let center_y = if cap { 0.25 } else { 0.5 };
        let center_top = if cap { 0.25 } else { 0.5 };
        let center_bottom = if cap { 0.75 } else { 0.5 };
        let uv_radius = if cap { 0.24 } else { 0.5 };
        let scale_top = if radius_top != 0.0 {
            uv_radius / radius_top
        } else if radius_bottom != 0.0 {
            uv_radius / radius_bottom
        } else {
            uv_radius
        };
        let scale_bottom = if radius_bottom != 0.0 {
            uv_radius / radius_bottom
        } else {
            scale_top
        };
        let up = crate::math::normalize(Mat3::from_mat4(self.matrix) * Vec3::Z);
        let (mut x, mut y) = (1.0f32, 1.0 - height);
        let mut ordered: Vec<FaceId> = faces.to_vec();
        ordered.sort_unstable();
        for face in ordered {
            let len = self.mesh.face_len(face);
            if len == 4 && radius_top != 0.0 && radius_bottom != 0.0 {
                let mut uvs = [Vec2::ZERO; 4];
                for (index, uv) in uvs.iter_mut().enumerate() {
                    match index {
                        1 => y += height,
                        2 => x -= width,
                        3 => y -= height,
                        _ => {}
                    }
                    *uv = Vec2::new(x, y);
                }
                self.set_uvs(face, &uvs);
            } else {
                let normal = self.mesh.calc_face_normal(face);
                self.mesh.set_face_normal(face, normal);
                let top = normal.dot(up) > 0.0;
                let uvs: Vec<Vec2> = self
                    .mesh
                    .face_verts(face)
                    .map(|vert| {
                        let local = self.local(self.mesh.position(vert));
                        match top {
                            true => Vec2::new(
                                center_top + local.x * scale_top,
                                center_y + local.y * scale_top,
                            ),
                            false => Vec2::new(
                                center_bottom + local.x * scale_bottom,
                                center_y + local.y * scale_bottom,
                            ),
                        }
                    })
                    .collect();
                self.set_uvs(face, &uvs);
            }
        }
    }

    fn uv_sphere(&mut self, segments: u32, rings: u32, radius: f32) -> Result<()> {
        let rings_i = rings as i32;
        let meridian: Vec<Vec3> = (0..=rings_i)
            .map(|ring| {
                let (sin, cos) = sin_cos_from_fraction(ring, 2 * rings_i);
                Vec3::new(0.0, radius * sin, radius * cos)
            })
            .collect();
        let center = meridian[rings as usize];
        let angle = -(PI * 2.0) / segments as f32;
        let rotation = Mat3::from_axis_angle(Vec3::Z, angle);
        let mut columns: Vec<Vec<Vec3>> = vec![meridian[1..rings as usize].to_vec()];
        for _ in 1..segments {
            let previous = columns.last().expect("first column exists");
            columns.push(
                previous
                    .iter()
                    .map(|&point| rotation * (point - center) + center)
                    .collect(),
            );
        }
        let north = self.raw_vert(meridian[0]);
        let mut ids: Vec<Vec<VertId>> = Vec::with_capacity(segments as usize);
        let first: Vec<VertId> = columns[0]
            .iter()
            .map(|&point| self.raw_vert(point))
            .collect();
        let south = self.raw_vert(meridian[rings as usize]);
        ids.push(first);
        for column in &columns[1..] {
            let column_ids: Vec<VertId> =
                column.iter().map(|&point| self.raw_vert(point)).collect();
            ids.push(column_ids);
        }
        let last_ring = rings as usize - 2;
        let mut faces = Vec::new();
        for column in 0..segments as usize {
            let here = &ids[column];
            let next = &ids[(column + 1) % segments as usize];
            faces.push(self.face(&[south, here[last_ring], next[last_ring]])?);
            for ring in 0..last_ring {
                faces.push(self.face(&[here[ring + 1], here[ring], next[ring], next[ring + 1]])?);
            }
            faces.push(self.face(&[here[0], north, next[0]])?);
        }
        if self.uv_map.is_some() {
            let mut all: Vec<(FaceId, Vec<Vec2>)> = faces
                .iter()
                .map(|&face| (face, self.sphere_face_uvs(face)))
                .collect();
            let min_x = all
                .iter()
                .flat_map(|(_, uvs)| uvs.iter().map(|uv| uv.x))
                .fold(1.0f32, f32::min);
            for (face, uvs) in &mut all {
                for uv in uvs.iter_mut() {
                    uv.x -= min_x;
                }
                self.set_uvs(*face, uvs);
            }
        }
        for vert in self.added.verts.clone() {
            let position = self.matrix.transform_point3(self.mesh.position(vert));
            self.mesh.set_position(vert, position);
        }
        Ok(())
    }

    fn sphere_face_uvs(&self, face: FaceId) -> Vec<Vec2> {
        let positions: Vec<Vec3> = self
            .mesh
            .face_verts(face)
            .map(|vert| self.mesh.position(vert))
            .collect();
        let triangle = positions.len() == 3;
        let (mut average_x, mut average_y) = (0.0f32, 0.0f32);
        if triangle {
            for point in &positions {
                average_x += point.x;
                average_y += point.y;
            }
        }
        average_x /= 3.0;
        average_y /= 3.0;
        let mut uvs: Vec<Vec2> = positions
            .iter()
            .map(|point| {
                let mut theta = match triangle && point.x.abs() < 0.0001 && point.y.abs() < 0.0001 {
                    true => average_y.atan2(average_x),
                    false => point.y.atan2(point.x),
                };
                if (theta - PI).abs() < 0.0001 {
                    theta = -PI;
                }
                let phi = (point.z / point.length()).clamp(-1.0, 1.0).acos();
                Vec2::new(0.5 + theta / (PI * 2.0), 1.0 - phi / PI)
            })
            .collect();
        let mut max_index = 0;
        for index in 1..uvs.len() {
            if uvs[index].x > uvs[max_index].x {
                max_index = index;
            }
        }
        let max_x = uvs[max_index].x;
        for (index, uv) in uvs.iter_mut().enumerate() {
            if index != max_index && max_x - uv.x > 0.5 {
                uv.x += 1.0;
            }
        }
        uvs
    }

    fn ico_sphere(&mut self, subdivisions: u32, radius: f32) -> Result<()> {
        let scale = radius / ICO_SCALE;
        let corners: Vec<Vec3> = ICO_VERTS
            .iter()
            .map(|&corner| Vec3::from_array(corner) * scale)
            .collect();
        let net_uv = |entry: [u8; 2]| {
            Vec2::new(
                entry[0] as f32 / ICO_UV_COLUMNS,
                entry[1] as f32 * ICO_UV_ROW,
            )
        };
        if subdivisions == 1 {
            let verts: Vec<VertId> = corners.iter().map(|&corner| self.vert(corner)).collect();
            for (face_corners, net) in ICO_FACES.iter().zip(ICO_NET) {
                let face = self.face(&face_corners.map(|corner| verts[corner]))?;
                self.set_uvs(face, &net.map(net_uv));
            }
            return Ok(());
        }
        let segments = 1usize << (subdivisions - 1);
        let mut lattice: std::collections::HashMap<[u64; 3], VertId> =
            std::collections::HashMap::new();
        let corner_ids: Vec<VertId> = corners.iter().map(|&corner| self.vert(corner)).collect();
        let mut lattice_vert =
            |builder: &mut Self, face: [usize; 3], weights: [usize; 3]| -> VertId {
                let nonzero: Vec<usize> = (0..3).filter(|&slot| weights[slot] != 0).collect();
                if nonzero.len() == 1 {
                    return corner_ids[face[nonzero[0]]];
                }
                let mut key_parts: Vec<(usize, usize)> = (0..3)
                    .map(|slot| (face[slot], weights[slot]))
                    .filter(|&(_, weight)| weight != 0)
                    .collect();
                key_parts.sort_unstable();
                let mut key = [u64::MAX; 3];
                for (slot, (corner, weight)) in key_parts.into_iter().enumerate() {
                    key[slot] = ((corner as u64) << 32) | weight as u64;
                }
                if let Some(&vert) = lattice.get(&key) {
                    return vert;
                }
                let flat: Vec3 = (0..3)
                    .map(|slot| corners[face[slot]] * (weights[slot] as f32 / segments as f32))
                    .sum();
                let vert = builder.vert(flat.normalize_or_zero() * radius);
                lattice.insert(key, vert);
                vert
            };
        for (face_corners, net) in ICO_FACES.iter().zip(ICO_NET) {
            let uv = net.map(net_uv);
            let mut rows: Vec<Vec<(VertId, [usize; 3])>> = Vec::with_capacity(segments + 1);
            for row in 0..=segments {
                let mut points = Vec::with_capacity(row + 1);
                for column in 0..=row {
                    let weights = [segments - row, row - column, column];
                    points.push((lattice_vert(self, *face_corners, weights), weights));
                }
                rows.push(points);
            }
            let interpolate = |weights: [usize; 3]| -> Vec2 {
                (0..3)
                    .map(|slot| uv[slot] * (weights[slot] as f32 / segments as f32))
                    .sum()
            };
            for row in 0..segments {
                for column in 0..=row {
                    let a = rows[row][column];
                    let b = rows[row + 1][column];
                    let c = rows[row + 1][column + 1];
                    let face = self.face(&[a.0, b.0, c.0])?;
                    self.set_uvs(
                        face,
                        &[interpolate(a.1), interpolate(b.1), interpolate(c.1)],
                    );
                    if column < row {
                        let d = rows[row][column + 1];
                        let face = self.face(&[a.0, c.0, d.0])?;
                        self.set_uvs(
                            face,
                            &[interpolate(a.1), interpolate(c.1), interpolate(d.1)],
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn torus(
        &mut self,
        major_segments: u32,
        minor_segments: u32,
        major_radius: f32,
        minor_radius: f32,
    ) -> Result<()> {
        let tau = std::f64::consts::TAU;
        let (major, minor) = (f64::from(major_radius), f64::from(minor_radius));
        let mut verts = Vec::with_capacity((major_segments * minor_segments) as usize);
        for major_index in 0..major_segments {
            let turn = f64::from(major_index) / f64::from(major_segments) * tau;
            let (sin, cos) = turn.sin_cos();
            for minor_index in 0..minor_segments {
                let angle = tau * f64::from(minor_index) / f64::from(minor_segments);
                let x = major + angle.cos() * minor;
                let z = angle.sin() * minor;
                let local = Vec3::new((cos * x) as f32, (sin * x) as f32, z as f32);
                verts.push(self.vert(local));
            }
        }
        let total = (major_segments * minor_segments) as usize;
        let minor_count = minor_segments as usize;
        let u_step = 1.0 / f64::from(major_segments);
        let v_step = 1.0 / f64::from(minor_segments);
        let u_init = 0.5 + 0.5f64 % u_step;
        let v_init = 0.5 + 0.5f64 % v_step;
        let u_wrap = 1.0 - u_step / 2.0;
        let v_wrap = 1.0 - v_step / 2.0;
        let mut u_prev = u_init;
        let mut u_next = u_prev + u_step;
        let mut first = 0usize;
        for major_index in 0..major_segments as usize {
            let mut v_prev = v_init;
            let mut v_next = v_prev + v_step;
            for minor_index in 0..minor_count {
                let (mut second, mut third, mut fourth) = match minor_index + 1 == minor_count {
                    true => {
                        let second = major_index * minor_count;
                        (second, first + minor_count, second + minor_count)
                    }
                    false => (first + 1, first + minor_count, first + minor_count + 1),
                };
                for value in [&mut second, &mut third, &mut fourth] {
                    if *value >= total {
                        *value -= total;
                    }
                }
                let face =
                    self.face(&[verts[first], verts[third], verts[fourth], verts[second]])?;
                let uvs = [
                    Vec2::new(u_prev as f32, v_prev as f32),
                    Vec2::new(u_next as f32, v_prev as f32),
                    Vec2::new(u_next as f32, v_next as f32),
                    Vec2::new(u_prev as f32, v_next as f32),
                ];
                self.set_uvs(face, &uvs);
                v_prev = if v_next > v_wrap {
                    v_next - 1.0
                } else {
                    v_next
                };
                v_next = v_prev + v_step;
                first += 1;
            }
            u_prev = if u_next > u_wrap {
                u_next - 1.0
            } else {
                u_next
            };
            u_next = u_prev + u_step;
        }
        Ok(())
    }
}
