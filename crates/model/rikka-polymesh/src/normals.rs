use std::f32::consts::{FRAC_PI_2, PI, TAU};

use glam::Vec3;
use rikka_model_core::polygon;

use crate::names::SHARP_EDGE;
use crate::parallel;
use crate::polymesh::Polymesh;
use crate::topology::GroupedIndices;

const TRIGO_THRESHOLD: f32 = 1.0 - 1.0e-4;
const SAME_NORMAL: f32 = 1.0e-4;
const GRAIN: usize = 1 << 13;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Shading {
    #[default]
    Split,
    Vertex,
}

#[must_use]
pub fn acos_approx(x: f32) -> f32 {
    let magnitude = x.abs();
    let m = if magnitude < 1.0 {
        1.0 - (1.0 - magnitude)
    } else {
        1.0
    };
    let a = (1.0 - m).sqrt()
        * (FRAC_PI_2 + m * (-0.213_300_99 + m * (0.077_980_48 + m * -0.021_640_95)));
    if x < 0.0 { PI - a } else { a }
}

#[must_use]
pub fn polygon_normal(positions: &[Vec3], corners: &[u32]) -> Vec3 {
    polygon::normal(corners.iter().map(|&point| positions[point as usize])).unwrap_or(Vec3::Z)
}

#[derive(Clone, Copy)]
enum Link {
    Open,
    One { corner: usize, towards: bool },
    Two { first: usize, second: usize },
    Sharp,
}

struct CornerInfo {
    face: usize,
    corner: usize,
    edge_previous: usize,
    edge_next: usize,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Space {
    normal: Vec3,
    reference: Vec3,
    ortho: Vec3,
    alpha: f32,
    beta: f32,
}

pub(crate) struct Fan {
    pub(crate) corners: Vec<u32>,
    pub(crate) space: Space,
}

struct Sharpness<'a> {
    edges: &'a [bool],
    faces: &'a [bool],
}

impl Sharpness<'_> {
    fn edge(&self, edge: usize) -> bool {
        self.edges.get(edge).copied().unwrap_or(false)
    }

    fn face(&self, face: usize) -> bool {
        self.faces.get(face).copied().unwrap_or(false)
    }
}

fn fan_space(normal: Vec3, reference: Vec3, other: Vec3, edges: &[Vec3]) -> Space {
    let reference_dot = reference.dot(normal);
    let other_dot = other.dot(normal);
    let mut space = Space {
        normal,
        reference: Vec3::ZERO,
        ortho: Vec3::ZERO,
        alpha: 0.0,
        beta: 0.0,
    };
    if reference_dot.abs() >= TRIGO_THRESHOLD || other_dot.abs() >= TRIGO_THRESHOLD {
        return space;
    }
    space.alpha = match edges.is_empty() {
        false => {
            edges
                .iter()
                .map(|edge| acos_approx(edge.dot(normal)))
                .sum::<f32>()
                / edges.len() as f32
        }
        true => (acos_approx(reference.dot(normal)) + acos_approx(other.dot(normal))) / 2.0,
    };
    space.reference = (reference - normal * reference_dot).normalize_or_zero();
    space.ortho = normal.cross(space.reference).normalize_or_zero();
    let projected = (other - normal * other_dot).normalize_or_zero();
    let dot = space.reference.dot(projected);
    space.beta = match dot < TRIGO_THRESHOLD {
        true => {
            let beta = acos_approx(dot);
            match space.ortho.dot(projected) < 0.0 {
                true => TAU - beta,
                false => beta,
            }
        }
        false => TAU,
    };
    space
}

fn decode(space: &Space, data: [i32; 2]) -> Vec3 {
    if data[0] == 0 || space.alpha == 0.0 || space.beta == 0.0 {
        return space.normal;
    }
    let unit = |value: i32| value as f32 / f32::from(i16::MAX);
    let alpha_factor = unit(data[0]);
    let alpha = match alpha_factor > 0.0 {
        true => space.alpha,
        false => TAU - space.alpha,
    } * alpha_factor;
    let beta_factor = unit(data[1]);
    let mut normal = space.normal * alpha.cos();
    if beta_factor == 0.0 {
        normal += space.reference * alpha.sin();
    } else {
        let beta = match beta_factor > 0.0 {
            true => space.beta,
            false => TAU - space.beta,
        } * beta_factor;
        normal += space.reference * (alpha.sin() * beta.cos());
        normal += space.ortho * (alpha.sin() * beta.sin());
    }
    normal
}

fn unit_to_short(value: f32) -> i16 {
    (value * f32::from(i16::MAX) + 0.5).floor() as i16
}

fn encode(space: &Space, custom: Vec3) -> [i16; 2] {
    if custom == Vec3::ZERO
        || (space.normal - custom).abs().max_element() <= SAME_NORMAL
        || space.alpha == 0.0
        || space.beta == 0.0
    {
        return [0, 0];
    }
    let cos_alpha = space.normal.dot(custom);
    let alpha = acos_approx(cos_alpha);
    let first = match alpha > space.alpha {
        true => unit_to_short(-(TAU - alpha) / (TAU - space.alpha)),
        false => unit_to_short(alpha / space.alpha),
    };
    let projected = (space.normal * -cos_alpha + custom).normalize_or_zero();
    let cos_beta = space.reference.dot(projected);
    let second = match cos_beta < TRIGO_THRESHOLD {
        true => {
            let mut beta = acos_approx(cos_beta);
            if space.ortho.dot(projected) < 0.0 {
                beta = TAU - beta;
            }
            match beta > space.beta {
                true => unit_to_short(-(TAU - beta) / (TAU - space.beta)),
                false => unit_to_short(beta / space.beta),
            }
        }
        false => 0,
    };
    [first, second]
}

fn corner_of(mesh: &Polymesh, face: usize, point: u32) -> usize {
    mesh.face(face)
        .find(|&corner| mesh.corner_verts[corner] == point)
        .expect("point face lists contain the point")
}

fn traverse(infos: &[CornerInfo], links: &[Link], start: usize, fan: &mut Vec<usize>) {
    fan.push(start);
    let mut current = start;
    let mut edge = infos[current].edge_next;
    let mut cyclic = false;
    while let Link::Two { first, second } = links[edge] {
        current = if current == first { second } else { first };
        if current == start {
            cyclic = true;
            break;
        }
        fan.push(current);
        edge = infos[current].edge_next;
    }
    fan.reverse();
    if cyclic {
        let lowest = fan
            .iter()
            .enumerate()
            .min_by_key(|(_, local)| infos[**local].corner)
            .map_or(0, |(index, _)| index);
        fan.rotate_left(lowest);
        return;
    }
    let mut current = start;
    let mut edge = infos[current].edge_previous;
    while let Link::Two { first, second } = links[edge] {
        current = if current == first { second } else { first };
        fan.push(current);
        edge = infos[current].edge_previous;
    }
}

impl Polymesh {
    #[must_use]
    pub fn face_normal(&self, face: usize) -> Vec3 {
        polygon_normal(&self.positions, self.face_verts(face))
    }

    #[must_use]
    pub fn face_normals(&self) -> Vec<Vec3> {
        parallel::collect(self.face_count(), GRAIN, |face| self.face_normal(face))
    }

    #[must_use]
    pub fn point_normals(&self) -> Vec<Vec3> {
        self.point_normals_from(&self.face_normals(), &self.point_faces())
    }

    pub(crate) fn point_normals_from(
        &self,
        faces: &[Vec3],
        point_faces: &GroupedIndices,
    ) -> Vec<Vec3> {
        parallel::collect(self.point_count(), GRAIN, |point| {
            let around = point_faces.get(point);
            let position = self.positions[point];
            if around.is_empty() {
                return position.normalize_or_zero();
            }
            let mut sum = Vec3::ZERO;
            for &face in around {
                let face = face as usize;
                let corner = corner_of(self, face, point as u32);
                let previous =
                    self.positions[self.corner_verts[self.previous_corner(face, corner)] as usize];
                let next =
                    self.positions[self.corner_verts[self.next_corner(face, corner)] as usize];
                let factor = acos_approx(
                    (previous - position)
                        .normalize_or_zero()
                        .dot((next - position).normalize_or_zero()),
                );
                sum += faces[face] * factor;
            }
            sum.normalize_or_zero()
        })
    }

    #[must_use]
    pub fn corner_normals(&self) -> Vec<Vec3> {
        self.shade(Shading::Split)
    }

    #[must_use]
    pub fn shade(&self, shading: Shading) -> Vec<Vec3> {
        if let Some(custom) = self.custom_normals() {
            return custom
                .iter()
                .map(|normal| normal.normalize_or_zero())
                .collect();
        }
        let faces = self.face_normals();
        let spread_faces = || -> Vec<Vec3> {
            (0..self.face_count())
                .flat_map(|face| std::iter::repeat_n(faces[face], self.face_size(face)))
                .collect()
        };
        let sharp_faces = self.sharp_faces().unwrap_or_default();
        let sharp_face_count = sharp_faces.iter().filter(|&&sharp| sharp).count();
        if let Shading::Vertex = shading {
            if sharp_face_count == self.face_count() {
                return spread_faces();
            }
            let points = self.point_normals_from(&faces, &self.point_faces());
            return (0..self.face_count())
                .flat_map(|face| {
                    let flat = self.is_sharp_face(face);
                    let (points, faces) = (&points, &faces);
                    self.face(face).map(move |corner| match flat {
                        true => faces[face],
                        false => points[self.corner_verts[corner] as usize],
                    })
                })
                .collect();
        }
        let sharp_edges = self.sharp_edges().unwrap_or_default();
        let sharp_edge_count = sharp_edges.iter().filter(|&&sharp| sharp).count();
        if sharp_face_count == self.face_count()
            || (sharp_edge_count == self.edge_count() && self.edge_count() > 0)
        {
            return spread_faces();
        }
        if sharp_face_count == 0 && sharp_edge_count == 0 {
            let points = self.point_normals_from(&faces, &self.point_faces());
            return parallel::collect(self.corner_count(), GRAIN, |corner| {
                points[self.corner_verts[corner] as usize]
            });
        }
        let sharpness = Sharpness {
            edges: sharp_edges,
            faces: sharp_faces,
        };
        self.split_normals(&faces, &sharpness, None, None)
    }

    #[must_use]
    pub fn decode_custom_normals(&self, encoded: &[[i16; 2]]) -> Vec<Vec3> {
        let faces = self.face_normals();
        let sharpness = Sharpness {
            edges: self.sharp_edges().unwrap_or_default(),
            faces: self.sharp_faces().unwrap_or_default(),
        };
        let encoded = (encoded.len() == self.corner_count()).then_some(encoded);
        self.split_normals(&faces, &sharpness, encoded, None)
    }

    pub fn encode_custom_normals(&mut self, normals: &[Vec3]) -> Vec<[i16; 2]> {
        let corners = self.corner_count();
        let faces = self.face_normals();
        let mut sharp_edges = self
            .sharp_edges()
            .map_or_else(|| vec![false; self.edge_count()], <[bool]>::to_vec);
        let sharp_faces = self.sharp_faces().unwrap_or_default().to_vec();
        let mut fans = Vec::new();
        let computed = self.split_normals(
            &faces,
            &Sharpness {
                edges: &sharp_edges,
                faces: &sharp_faces,
            },
            None,
            Some(&mut fans),
        );
        let custom: Vec<Vec3> = (0..corners)
            .map(|corner| match normals.get(corner).copied() {
                Some(normal) if normal != Vec3::ZERO => normal,
                _ => computed[corner],
            })
            .collect();
        let owners = self.corner_faces();
        let mut sharpened = false;
        for fan in &fans {
            let Some((&last, _)) = fan.corners.split_last() else {
                continue;
            };
            let mut previous: Option<usize> = None;
            let mut reference: Option<Vec3> = None;
            let split = |corner: usize, previous: usize, sharp_edges: &mut [bool]| {
                let face = owners[corner] as usize;
                let corner_previous = self.previous_corner(face, corner);
                let edge = self.corner_edges[corner] as usize;
                let edge_previous = self.corner_edges[corner_previous] as usize;
                let previous_edge = self.corner_edges[previous] as usize;
                let target = match previous_edge == edge_previous {
                    true => previous_edge,
                    false => edge,
                };
                sharp_edges[target] = true;
            };
            for &corner in fan.corners.iter().rev() {
                let corner = corner as usize;
                let normal = custom[corner];
                match reference {
                    None => reference = Some(normal),
                    Some(original) if original.dot(normal) < TRIGO_THRESHOLD => {
                        if let Some(previous) = previous {
                            split(corner, previous, &mut sharp_edges);
                            sharpened = true;
                        }
                        reference = Some(normal);
                    }
                    Some(_) => {}
                }
                previous = Some(corner);
            }
            if fan.corners.len() > 1
                && let (Some(original), Some(previous)) = (reference, previous)
                && original.dot(custom[last as usize]) < TRIGO_THRESHOLD
            {
                split(last as usize, previous, &mut sharp_edges);
                sharpened = true;
            }
        }
        if sharpened {
            fans.clear();
            self.split_normals(
                &faces,
                &Sharpness {
                    edges: &sharp_edges,
                    faces: &sharp_faces,
                },
                None,
                Some(&mut fans),
            );
            self.sharp_edges_mut().copy_from_slice(&sharp_edges);
        }
        let mut encoded = vec![[0i16; 2]; corners];
        for fan in &fans {
            let average = match fan.corners.len() {
                1 => custom[fan.corners[0] as usize],
                count => {
                    fan.corners
                        .iter()
                        .map(|&corner| custom[corner as usize])
                        .sum::<Vec3>()
                        / count as f32
                }
            };
            let data = encode(&fan.space, average);
            for &corner in &fan.corners {
                encoded[corner as usize] = data;
            }
        }
        encoded
    }

    pub fn sharpen_by_angle(&mut self, angle: f32, keep_sharp_edges: bool) {
        if angle >= PI {
            self.set_smooth(true, keep_sharp_edges);
            return;
        }
        if angle == 0.0 {
            self.set_smooth(false, keep_sharp_edges);
            return;
        }
        if !keep_sharp_edges {
            self.attributes.remove(SHARP_EDGE);
        }
        self.ensure_edges();
        const UNSET: i64 = i64::MIN;
        const INVALID: i64 = -1;
        let threshold = angle.cos();
        let faces = self.face_normals();
        let owners = self.corner_faces();
        let sharp_faces = self.sharp_faces().unwrap_or_default().to_vec();
        let mut edge_corners = vec![[0i64; 2]; self.edge_count()];
        let corner_verts = self.corner_verts.clone();
        let corner_edges = self.corner_edges.clone();
        let face_ranges: Vec<std::ops::Range<usize>> =
            (0..self.face_count()).map(|face| self.face(face)).collect();
        let sharp_edges = self.sharp_edges_mut();
        for (face, range) in face_ranges.into_iter().enumerate() {
            let smooth = !sharp_faces.get(face).copied().unwrap_or(false);
            for corner in range {
                let point = corner_verts[corner];
                let edge = corner_edges[corner] as usize;
                let state = &mut edge_corners[edge];
                if state[0] == 0 && state[1] == 0 {
                    state[0] = corner as i64;
                    state[1] = if smooth { UNSET } else { INVALID };
                } else if state[1] == UNSET {
                    let first = state[0] as usize;
                    let by_angle = faces[owners[first] as usize].dot(faces[face]) < threshold;
                    if !smooth || sharp_edges[edge] || point == corner_verts[first] || by_angle {
                        state[1] = INVALID;
                        if by_angle {
                            sharp_edges[edge] = true;
                        }
                    } else {
                        state[1] = corner as i64;
                    }
                } else if !(state[1] == UNSET || state[1] == INVALID) {
                    state[1] = INVALID;
                    sharp_edges[edge] = false;
                }
            }
        }
    }

    pub fn set_smooth(&mut self, smooth: bool, keep_sharp_edges: bool) {
        if !keep_sharp_edges {
            self.attributes.remove(SHARP_EDGE);
        }
        self.attributes.remove(crate::names::SHARP_FACE);
        if !smooth {
            self.sharp_faces_mut().fill(true);
        }
    }

    fn split_normals(
        &self,
        faces: &[Vec3],
        sharpness: &Sharpness<'_>,
        custom: Option<&[[i16; 2]]>,
        mut fans: Option<&mut Vec<Fan>>,
    ) -> Vec<Vec3> {
        let mut normals = vec![Vec3::ZERO; self.corner_count()];
        let point_faces = self.point_faces();
        let mut infos: Vec<CornerInfo> = Vec::new();
        let mut others: Vec<u32> = Vec::new();
        let mut links: Vec<Link> = Vec::new();
        let mut directions: Vec<Vec3> = Vec::new();
        let mut visited: Vec<bool> = Vec::new();
        let mut fan: Vec<usize> = Vec::new();
        let spaces_needed = custom.is_some() || fans.is_some();

        for point in 0..self.point_count() {
            let around = point_faces.get(point);
            if around.is_empty() {
                continue;
            }
            infos.clear();
            others.clear();
            let local_edge = |other: u32, others: &mut Vec<u32>| match others
                .iter()
                .position(|&candidate| candidate == other)
            {
                Some(index) => index,
                None => {
                    others.push(other);
                    others.len() - 1
                }
            };
            for &face in around {
                let face = face as usize;
                let corner = corner_of(self, face, point as u32);
                let previous = self.corner_verts[self.previous_corner(face, corner)];
                let next = self.corner_verts[self.next_corner(face, corner)];
                let edge_previous = local_edge(previous, &mut others);
                let edge_next = local_edge(next, &mut others);
                infos.push(CornerInfo {
                    face,
                    corner,
                    edge_previous,
                    edge_next,
                });
            }

            links.clear();
            links.resize(others.len(), Link::Open);
            for (local, info) in infos.iter().enumerate() {
                if sharpness.face(info.face) {
                    links[info.edge_previous] = Link::Sharp;
                    links[info.edge_next] = Link::Sharp;
                    continue;
                }
                let previous_corner = self.previous_corner(info.face, info.corner);
                for (edge, towards, other) in [
                    (info.edge_previous, true, previous_corner),
                    (
                        info.edge_next,
                        false,
                        self.next_corner(info.face, info.corner),
                    ),
                ] {
                    links[edge] = match links[edge] {
                        Link::Open => {
                            let mesh_edge =
                                self.corner_edges[if towards { other } else { info.corner }];
                            match sharpness.edge(mesh_edge as usize) {
                                true => Link::Sharp,
                                false => Link::One {
                                    corner: local,
                                    towards,
                                },
                            }
                        }
                        Link::One {
                            corner,
                            towards: first_towards,
                        } => match first_towards == towards {
                            true => Link::Sharp,
                            false => Link::Two {
                                first: corner,
                                second: local,
                            },
                        },
                        _ => Link::Sharp,
                    };
                }
            }

            let position = self.positions[point];
            directions.clear();
            directions.extend(
                others
                    .iter()
                    .map(|&other| (self.positions[other as usize] - position).normalize_or_zero()),
            );

            visited.clear();
            visited.resize(infos.len(), false);
            let mut seen = 0;
            let mut start = 0;
            loop {
                fan.clear();
                traverse(&infos, &links, start, &mut fan);
                let mut normal = match fan.len() {
                    1 => faces[infos[fan[0]].face],
                    _ => fan
                        .iter()
                        .map(|&local| {
                            let info = &infos[local];
                            faces[info.face]
                                * acos_approx(
                                    directions[info.edge_previous].dot(directions[info.edge_next]),
                                )
                        })
                        .sum::<Vec3>()
                        .normalize_or_zero(),
                };
                if spaces_needed {
                    let first = infos[fan[0]].edge_next;
                    let last =
                        infos[*fan.last().expect("fans have at least one corner")].edge_previous;
                    let mut edges = Vec::new();
                    if fan.len() > 1 {
                        edges.extend(fan.iter().map(|&local| directions[infos[local].edge_next]));
                        if last != first {
                            edges.push(directions[last]);
                        }
                    }
                    let space = fan_space(normal, directions[first], directions[last], &edges);
                    if let Some(custom) = custom {
                        let mut sum = [0i32; 2];
                        for &local in &fan {
                            let data = custom[infos[local].corner];
                            sum[0] += i32::from(data[0]);
                            sum[1] += i32::from(data[1]);
                        }
                        let count = fan.len() as i32;
                        normal = decode(&space, [sum[0] / count, sum[1] / count]);
                    }
                    if let Some(fans) = fans.as_deref_mut() {
                        fans.push(Fan {
                            corners: fan
                                .iter()
                                .map(|&local| infos[local].corner as u32)
                                .collect(),
                            space,
                        });
                    }
                }
                for &local in &fan {
                    normals[infos[local].corner] = normal;
                    visited[local] = true;
                }
                seen += fan.len();
                if seen >= infos.len() {
                    break;
                }
                while visited[start] {
                    start += 1;
                }
            }
        }
        normals
    }
}
