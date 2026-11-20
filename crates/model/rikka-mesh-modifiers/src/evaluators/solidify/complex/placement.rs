use std::f32::consts::PI;

use glam::{Mat3, Vec3, Vec4};
use rikka_polymesh::Polymesh;

use super::{EdgeGroup, Settings, Topology, normalize_length};
use crate::evaluators::vertex_group::Group;
use crate::kinds::solidify::{BoundaryShape, Solidify, ThicknessMode};
use crate::math::angle;

const BOUNDARY_FIX_THRESHOLD: f32 = 0.7;
const MATRIX_EPSILON: f32 = 5.0e-5;
const EQUAL_PLANES: f32 = 0.98;

struct Context<'a> {
    solidify: &'a Solidify,
    topology: &'a Topology,
    mesh: &'a Polymesh,
    settings: &'a Settings,
    face_weights: Option<Vec<f32>>,
}

fn invert(matrix: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let m = matrix;
    let mut adjoint = [
        [
            m[1][1] * m[2][2] - m[1][2] * m[2][1],
            -m[0][1] * m[2][2] + m[0][2] * m[2][1],
            m[0][1] * m[1][2] - m[0][2] * m[1][1],
        ],
        [
            -m[1][0] * m[2][2] + m[1][2] * m[2][0],
            m[0][0] * m[2][2] - m[0][2] * m[2][0],
            -m[0][0] * m[1][2] + m[0][2] * m[1][0],
        ],
        [
            m[1][0] * m[2][1] - m[1][1] * m[2][0],
            -m[0][0] * m[2][1] + m[0][1] * m[2][0],
            m[0][0] * m[1][1] - m[0][1] * m[1][0],
        ],
    ];
    let determinant = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[1][0] * (m[0][1] * m[2][2] - m[0][2] * m[2][1])
        + m[2][0] * (m[0][1] * m[1][2] - m[0][2] * m[1][1]);
    if determinant != 0.0 {
        let scale = 1.0 / determinant;
        for row in &mut adjoint {
            for value in row {
                *value *= scale;
            }
        }
    }
    adjoint
}

fn transform(matrix: [[f32; 3]; 3], vector: Vec3) -> Vec3 {
    Mat3::from_cols(
        Vec3::from(matrix[0]),
        Vec3::from(matrix[1]),
        Vec3::from(matrix[2]),
    ) * vector
}

impl Context<'_> {
    fn far_vertex(&self, old_edge: u32, vert: usize) -> Vec3 {
        let topology = self.topology;
        let [a, b] = self.mesh.edges[old_edge as usize];
        let far = match topology.vm[a as usize] as usize == vert {
            true => b,
            false => a,
        };
        topology.coords[topology.vm[far as usize] as usize]
    }

    fn side_normal(&self, side: u32) -> Vec3 {
        let normal = self.topology.face_normals[side as usize / 2];
        match side & 1 == 1 {
            true => -normal,
            false => normal,
        }
    }

    fn sides(&self, group: &EdgeGroup) -> Vec<u32> {
        let topology = self.topology;
        let cycle = (group.is_orig_closed && group.split == 0) || group.is_even_split;
        let len = group.edges.len();
        let mut first: Option<usize> = None;
        let mut sides = Vec::new();
        for (k, &edge) in group.edges.iter().enumerate() {
            if k & 1 == 0 || (!cycle && k == len - 1) {
                let faces = topology.edges[edge as usize].faces;
                for side in faces.into_iter().flatten() {
                    let excluded = first.is_some_and(|first| {
                        let faces = topology.edges[group.edges[first] as usize].faces;
                        faces[0] == Some(side) || faces[1] == Some(side)
                    });
                    if !excluded {
                        sides.push(side);
                    }
                }
                if (cycle && k == 0) || (!cycle && k + 3 >= len) {
                    first = Some(k);
                }
            }
        }
        sides
    }

    fn side_offset(&self, side: u32) -> f32 {
        let mut offset = match side & 1 == 1 {
            true => self.settings.ofs_back_clamped,
            false => self.settings.ofs_front_clamped,
        };
        if let Some(weights) = &self.face_weights {
            offset *= weights[side as usize / 2];
        }
        offset
    }

    fn constraints(
        &self,
        group: &EdgeGroup,
        normal: &mut Vec3,
        move_normal: &mut Vec3,
        fix: &mut Fix,
    ) {
        let topology = self.topology;
        let mut planes: Vec<Vec4> = Vec::new();
        let mut fallback = Vec4::ZERO;
        for side in self.sides(group) {
            let offset = self.side_offset(side);
            let plane = self.side_normal(side).extend(offset);
            match topology.null_faces[side as usize / 2] {
                false => planes.push(plane),
                true => fallback = plane,
            }
        }
        if planes.len() > 2 {
            let mut min_p = 2.0;
            let (mut n0, mut n1) = (0, 0);
            for k in 0..planes.len() {
                for m in k + 1..planes.len() {
                    let p = planes[k].truncate().dot(planes[m].truncate());
                    if p < min_p {
                        min_p = p;
                        n0 = k;
                        n1 = m;
                    }
                }
            }
            if n1 != 0 {
                planes.swap(n0, 0);
                planes.swap(n1, 1);
            } else {
                planes.swap(n0, 1);
            }
            let mut min_p = 1.0;
            let mut third = 2;
            for k in 2..planes.len() {
                let max_p = planes[0]
                    .truncate()
                    .dot(planes[k].truncate())
                    .max(planes[1].truncate().dot(planes[k].truncate()));
                if max_p <= min_p {
                    min_p = max_p;
                    third = k;
                }
            }
            planes.swap(third, 2);
        }
        while planes.len() > 2 {
            let (mut best0, mut best1) = (0, 0);
            let mut best_p = -1.0;
            let mut best_diff = 0.0;
            for k in 0..planes.len() {
                for m in k + 1..planes.len() {
                    let p = planes[m].truncate().dot(planes[k].truncate());
                    let diff = (planes[m].w - planes[k].w).abs();
                    if p > best_p + f32::EPSILON || (p >= best_p && diff < best_diff) {
                        best_p = p;
                        best_diff = diff;
                        best0 = k;
                        best1 = m;
                    }
                }
            }
            if best_p < EQUAL_PLANES {
                break;
            }
            let (merged, _) = normalize_length(planes[best0].truncate() + planes[best1].truncate());
            let offset = (planes[best0].w + planes[best1].w) * 0.5;
            planes[best0] = merged.extend(offset);
            planes.remove(best1);
        }
        let size = planes.len();
        if size > 3 {
            let mut matrix = [[0.0f32; 3]; 3];
            for (k, row) in matrix.iter_mut().enumerate() {
                for plane in &planes {
                    let scaled = plane.truncate() * plane[k];
                    row[0] += scaled.x;
                    row[1] += scaled.y;
                    row[2] += scaled.z;
                }
                row[k] += MATRIX_EPSILON;
            }
            let inverse = invert(matrix);
            let mut sum = Vec3::ZERO;
            for plane in &planes {
                sum += plane.truncate() * plane.w;
            }
            *normal = transform(inverse, sum);
            if !fix.disabled {
                let mut greatest = 1.0f32;
                for k in 0..2 {
                    for plane in &planes[2..] {
                        greatest = greatest.min(plane.truncate().dot(planes[k].truncate()));
                    }
                }
                match greatest > BOUNDARY_FIX_THRESHOLD {
                    true => fix.approximate = true,
                    false => fix.disabled = true,
                }
            }
        } else if size > 1 {
            let stop_explosion = 0.999 - self.solidify.offset.abs() * 0.05;
            let (first, second) = (planes[0].truncate(), planes[1].truncate());
            let q = first.dot(second);
            let mut d = 1.0 - q * q;
            *move_normal = normalize_length(first.cross(second)).0;
            let (scaled_first, scaled_second) = if d > f32::EPSILON * 10.0 && q < stop_explosion {
                d = 1.0 / d;
                (
                    first * ((planes[0].w - planes[1].w * q) * d),
                    second * ((planes[1].w - planes[0].w * q) * d),
                )
            } else {
                d = 1.0 / (q.abs() + 1.0);
                (first * (planes[0].w * d), second * (planes[1].w * d))
            };
            *normal = scaled_first + scaled_second;
            if size == 3 {
                let third = planes[2].truncate();
                let d = third.dot(*move_normal);
                if d.abs() > 0.02 {
                    let toward = *normal - third * planes[2].w;
                    *normal -= *move_normal * (third.dot(toward) / d);
                    if d.abs() > 1.0 - BOUNDARY_FIX_THRESHOLD {
                        fix.disabled = true;
                    }
                }
            }
            fix.approximate = false;
        } else if size == 1 {
            *normal = planes[0].truncate() * planes[0].w;
            if group.edges.len() > 2 {
                fix.disabled = true;
                fix.approximate = true;
            }
        } else {
            *normal = fallback.truncate() * fallback.w;
            fix.disabled = true;
        }
    }

    fn fixed_or_even(&self, group: &EdgeGroup, vert: usize, normal: &mut Vec3, fix: &mut Fix) {
        let topology = self.topology;
        let mesh = self.mesh;
        let even = self.solidify.nonmanifold_thickness_mode == ThicknessMode::Even;
        let mut total = 0.0f32;
        let mut total_back = 0.0f32;
        let mut back = Vec3::ZERO;
        let (mut has_back, mut has_front) = (false, false);
        for side in self.sides(group) {
            let reversed = side & 1 == 1;
            let face = side as usize / 2;
            let mut weight = 1.0;
            let offset = match reversed {
                true => -self.side_offset(side),
                false => self.side_offset(side),
            };
            if even {
                let range = mesh.face(face);
                let size = range.len();
                let mut next = range.start;
                let mut corner = range.start + size - 1;
                let mut previous = corner - 1;
                let mut m = 0;
                while m < size && topology.vm[mesh.corner_verts[corner] as usize] as usize != vert {
                    previous = corner;
                    corner = next;
                    m += 1;
                    next += 1;
                }
                let at = |corner: usize| {
                    topology.coords[topology.vm[mesh.corner_verts[corner] as usize] as usize]
                };
                let origin = topology.coords[vert];
                weight = angle(at(previous) - origin, at(next) - origin);
                match reversed {
                    true => total_back += weight * offset * offset,
                    false => total += weight * offset * offset,
                }
            } else {
                match reversed {
                    true => total_back += 1.0,
                    false => total += 1.0,
                }
            }
            let contribution = topology.face_normals[face] * (weight * offset);
            match reversed {
                true => {
                    back += contribution;
                    has_back = true;
                }
                false => {
                    *normal += contribution;
                    has_front = true;
                }
            }
        }
        if even {
            if has_front {
                let squared = normal.length_squared();
                if squared > f32::EPSILON {
                    *normal *= total / squared;
                }
            }
            if has_back {
                let squared = back.length_squared();
                if squared > f32::EPSILON {
                    back *= total_back / squared;
                }
                if !has_front {
                    *normal = back;
                }
            }
            if has_front && has_back {
                let normal_length = normal.length();
                let back_length = back.length();
                let mut q = normal.dot(back);
                if q.abs() > f32::EPSILON {
                    q /= normal_length * back_length;
                }
                let mut d = 1.0 - q * q;
                if d > f32::EPSILON {
                    d = 1.0 / d;
                    if normal_length > f32::EPSILON {
                        *normal *= (1.0 - back_length * q / normal_length) * d;
                    }
                    if back_length > f32::EPSILON {
                        back *= (1.0 - normal_length * q / back_length) * d;
                    }
                    *normal += back;
                } else {
                    *normal *= 0.5;
                    back *= 0.5;
                    *normal += back;
                }
            }
        } else {
            if has_front && total > f32::EPSILON {
                *normal *= 1.0 / total;
            }
            if has_back && total_back > f32::EPSILON {
                back *= 1.0 / total_back;
                *normal += back;
                if has_front && total > f32::EPSILON {
                    *normal *= 0.5;
                }
            }
        }
        match !fix.disabled && group.edges.len() > 2 {
            true => fix.approximate = true,
            false => fix.disabled = true,
        }
    }

    fn position(&self, group: &EdgeGroup, vert: usize) -> Vec3 {
        let topology = self.topology;
        let origin = topology.coords[vert];
        if group.is_singularity {
            return origin;
        }
        let mut normal = Vec3::ZERO;
        let mut move_normal = Vec3::ZERO;
        let mut fix = Fix {
            disabled: self.solidify.nonmanifold_boundary_mode == BoundaryShape::None
                || group.is_orig_closed
                || group.split != 0,
            approximate: false,
        };
        match self.solidify.nonmanifold_thickness_mode {
            ThicknessMode::Constraints => {
                self.constraints(group, &mut normal, &mut move_normal, &mut fix);
            }
            ThicknessMode::Fixed | ThicknessMode::Even => {
                self.fixed_or_even(group, vert, &mut normal, &mut fix);
            }
        }
        if fix.approximate {
            let len = group.edges.len();
            for &edge in group.edges.iter().take(len.saturating_sub(1)).skip(1) {
                move_normal +=
                    self.far_vertex(topology.edges[edge as usize].old_edge, vert) - origin;
            }
            fix.disabled = match len > 2 {
                false => true,
                true => {
                    let (normalized, length) = normalize_length(move_normal);
                    move_normal = normalized;
                    length == 0.0
                }
            };
        }
        if !fix.disabled {
            let first = &topology.edges[group.edges[0] as usize];
            let last = &topology.edges[group.edges[group.edges.len() - 1] as usize];
            let e0 = self.far_vertex(first.old_edge, vert) - origin;
            let e1 = self.far_vertex(last.old_edge, vert) - origin;
            let constraint = match self.solidify.nonmanifold_boundary_mode {
                BoundaryShape::Flat => normalize_length(e0.cross(e1)).0,
                BoundaryShape::Round | BoundaryShape::None => {
                    let f0 = first.faces[0].map_or(Vec3::ZERO, |side| self.side_normal(side));
                    let f1 = last.faces[0].map_or(Vec3::ZERO, |side| self.side_normal(side));
                    let n0 = normalize_length(e0.cross(f0)).0;
                    let n1 = normalize_length(f1.cross(e1)).0;
                    normalize_length(n0 + n1).0
                }
            };
            let d = constraint.dot(move_normal);
            if d.abs() > 0.1 {
                move_normal *= constraint.dot(normal) / d;
                normal -= move_normal;
            }
        }
        let mut scale = 1.0;
        if let Group::Weighted(_) = self.settings.group
            && self.face_weights.is_none()
        {
            let weight = self
                .settings
                .group
                .weight(self.mesh, vert, self.settings.invert);
            scale = self.solidify.thickness_vertex_group
                + weight * (1.0 - self.solidify.thickness_vertex_group);
        }
        let offset = self.solidify.thickness.abs() * self.solidify.thickness_clamp;
        if self.solidify.thickness_clamp != 0.0 {
            let lengths = group.edges.iter().map(|&edge| {
                topology.edge_lengths[topology.edges[edge as usize].old_edge as usize]
            });
            if self.solidify.use_thickness_angle_clamp {
                if group.edges.len() > 2 {
                    let min_length = lengths.fold(f32::INFINITY, f32::min);
                    let angle = group
                        .edges
                        .iter()
                        .map(|&edge| topology.edges[edge as usize].angle)
                        .fold(0.5 * PI, f32::max);
                    let cosine = (angle * 0.5).cos();
                    if cosine > 0.0 {
                        let most = min_length * 0.5 / cosine;
                        if most < offset * 0.5 {
                            scale *= most / offset * 2.0;
                        }
                    }
                }
            } else {
                let min_length = lengths.fold(f32::INFINITY, f32::min);
                if min_length < offset {
                    scale *= min_length / offset;
                }
            }
        }
        normal * scale + origin
    }
}

struct Fix {
    disabled: bool,
    approximate: bool,
}

pub(super) fn place(
    solidify: &Solidify,
    topology: &mut Topology,
    mesh: &Polymesh,
    settings: &Settings,
) {
    let face_weights = settings.flat_faces.then(|| {
        (0..mesh.face_count())
            .map(|face| {
                let weight = mesh
                    .face_verts(face)
                    .iter()
                    .map(|&vert| settings.group.weight(mesh, vert as usize, settings.invert))
                    .fold(1.0f32, f32::min);
                solidify.thickness_vertex_group + weight * (1.0 - solidify.thickness_vertex_group)
            })
            .collect()
    });
    let positions: Vec<Vec<Vec3>> = {
        let context = Context {
            solidify,
            topology,
            mesh,
            settings,
            face_weights,
        };
        topology
            .groups
            .iter()
            .enumerate()
            .map(|(vert, groups)| {
                groups
                    .iter()
                    .map(|group| context.position(group, vert))
                    .collect()
            })
            .collect()
    };
    for (groups, positions) in topology.groups.iter_mut().zip(positions) {
        for (group, position) in groups.iter_mut().zip(positions) {
            group.co = position;
        }
    }
}
