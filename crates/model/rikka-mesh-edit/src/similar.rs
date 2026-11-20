use std::collections::BTreeSet;
use std::f32::consts::PI;

use glam::{Mat3, Mat4, Vec3, Vec4};
use rikka_polymesh::Domain;

use crate::ids::{EdgeId, FaceId, VertId};
use crate::math::newell_from;
use crate::math::{angle_normalized, angle_vectors, normalize};
use crate::mesh::EditMesh;
use crate::select_ops::Compare;

const FREESTYLE_EDGE: &str = "freestyle_edge";
const FREESTYLE_FACE: &str = "freestyle_face";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SimilarRegions {
    TooFewFaces,
    Found(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SimilarVert {
    Normal,
    Faces,
    Groups,
    Edges,
    Crease,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SimilarEdge {
    Length,
    Direction,
    Faces,
    FaceAngle,
    Crease,
    Bevel,
    Seam,
    Sharp,
    Freestyle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SimilarFace {
    Material,
    Area,
    Sides,
    Perimeter,
    Normal,
    Coplanar,
    Smooth,
    Freestyle,
}

fn compare_int(delta: i64, compare: Compare) -> bool {
    match compare {
        Compare::Equal => delta == 0,
        Compare::NotEqual => delta != 0,
        Compare::Greater => delta > 0,
        Compare::Less => delta < 0,
    }
}

fn compare_float(delta: f32, threshold: f32, compare: Compare) -> bool {
    match compare {
        Compare::Equal => delta.abs() <= threshold,
        Compare::NotEqual => delta.abs() > threshold,
        Compare::Greater => delta + threshold >= 0.0,
        Compare::Less => delta - threshold <= 0.0,
    }
}

fn compare_float_set(values: &[f32], value: f32, threshold: f32, compare: Compare) -> bool {
    let reference = match compare {
        Compare::Equal | Compare::NotEqual => values
            .iter()
            .copied()
            .min_by(|a, b| (a - value).abs().total_cmp(&(b - value).abs())),
        Compare::Greater => values.iter().copied().min_by(f32::total_cmp),
        Compare::Less => values.iter().copied().max_by(f32::total_cmp),
    };
    reference.is_some_and(|reference| compare_float(value - reference, threshold, compare))
}

fn nearest_angle(values: &[Vec3], value: Vec3) -> Option<f32> {
    values
        .iter()
        .min_by(|a, b| {
            a.distance_squared(value)
                .total_cmp(&b.distance_squared(value))
        })
        .map(|&nearest| angle_normalized(value, nearest))
}

fn canonical_direction(direction: Vec3) -> Vec3 {
    let flip = if direction.z.abs() < f32::EPSILON {
        if direction.y.abs() < f32::EPSILON {
            direction.x < 0.0
        } else {
            direction.y < 0.0
        }
    } else {
        direction.z < 0.0
    };
    match flip {
        true => -direction,
        false => direction,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SimilarValue {
    Count(i64),
    Scalar(f32),
    Direction(Vec3),
    Plane(Vec4),
    Flag(bool),
    Material(i32),
    Groups(Vec<String>),
}

struct Space {
    matrix: Mat4,
    normal: Mat3,
    identity: bool,
}

impl Space {
    fn new(matrix: &Mat4) -> Space {
        Space {
            matrix: *matrix,
            normal: Mat3::from_mat4(*matrix).inverse().transpose(),
            identity: *matrix == Mat4::IDENTITY,
        }
    }

    fn point(&self, point: Vec3) -> Vec3 {
        match self.identity {
            true => point,
            false => self.matrix.transform_point3(point),
        }
    }

    fn normal(&self, normal: Vec3) -> Vec3 {
        match self.identity {
            true => normal,
            false => normalize(self.normal * normal),
        }
    }
}

fn counts(references: &[SimilarValue]) -> BTreeSet<i64> {
    references
        .iter()
        .filter_map(|value| match value {
            SimilarValue::Count(count) => Some(*count),
            _ => None,
        })
        .collect()
}

fn scalars(references: &[SimilarValue]) -> Vec<f32> {
    references
        .iter()
        .filter_map(|value| match value {
            SimilarValue::Scalar(scalar) => Some(*scalar),
            _ => None,
        })
        .collect()
}

fn directions(references: &[SimilarValue]) -> Vec<Vec3> {
    references
        .iter()
        .filter_map(|value| match value {
            SimilarValue::Direction(direction) => Some(*direction),
            _ => None,
        })
        .collect()
}

fn flags(references: &[SimilarValue]) -> BTreeSet<bool> {
    references
        .iter()
        .filter_map(|value| match value {
            SimilarValue::Flag(flag) => Some(*flag),
            _ => None,
        })
        .collect()
}

impl EditMesh {
    fn bool_attribute(&self, name: &str, domain: Domain, index: usize) -> bool {
        self.attribute::<bool>(name, domain)
            .is_some_and(|values| values[index])
    }

    fn vert_group_names(&self, vert: VertId) -> Vec<String> {
        let groups = self.vertex_groups();
        self.vert_influences(vert)
            .iter()
            .filter(|influence| influence.weight > 0.0)
            .filter_map(|influence| groups.get(influence.group as usize))
            .map(|group| group.name.clone())
            .collect()
    }

    #[must_use]
    pub fn similar_vert_value(
        &self,
        vert: VertId,
        kind: SimilarVert,
        matrix: &Mat4,
    ) -> SimilarValue {
        let space = Space::new(matrix);
        match kind {
            SimilarVert::Normal => SimilarValue::Direction(space.normal(self.vert_normal(vert))),
            SimilarVert::Faces => SimilarValue::Count(self.vert_face_count(vert) as i64),
            SimilarVert::Edges => SimilarValue::Count(self.vert_edge_count(vert) as i64),
            SimilarVert::Groups => SimilarValue::Groups(self.vert_group_names(vert)),
            SimilarVert::Crease => SimilarValue::Scalar(
                self.vert_creases()
                    .map_or(0.0, |values| values[vert.index()]),
            ),
        }
    }

    #[must_use]
    pub fn similar_edge_value(
        &self,
        edge: EdgeId,
        kind: SimilarEdge,
        matrix: &Mat4,
    ) -> Option<SimilarValue> {
        let space = Space::new(matrix);
        let [a, b] = self
            .edge_verts(edge)
            .map(|vert| space.point(self.position(vert)));
        Some(match kind {
            SimilarEdge::Length => SimilarValue::Scalar(a.distance_squared(b)),
            SimilarEdge::Direction => {
                SimilarValue::Direction(canonical_direction(normalize(a - b)))
            }
            SimilarEdge::Faces => SimilarValue::Count(self.edge_face_count(edge) as i64),
            SimilarEdge::FaceAngle => {
                let (first, second) = self.edge_loop_pair(edge)?;
                SimilarValue::Scalar(angle_normalized(
                    space.normal(self.face_normal(self.loop_face(first))),
                    space.normal(self.face_normal(self.loop_face(second))),
                ))
            }
            SimilarEdge::Crease => SimilarValue::Scalar(
                self.edge_creases()
                    .map_or(0.0, |values| values[edge.index()]),
            ),
            SimilarEdge::Bevel => SimilarValue::Scalar(
                self.edge_bevel_weights()
                    .map_or(0.0, |values| values[edge.index()]),
            ),
            SimilarEdge::Seam => SimilarValue::Flag(self.is_edge_seam(edge)),
            SimilarEdge::Sharp => SimilarValue::Flag(!self.is_edge_sharp(edge)),
            SimilarEdge::Freestyle => {
                SimilarValue::Flag(self.bool_attribute(FREESTYLE_EDGE, Domain::Edge, edge.index()))
            }
        })
    }

    #[must_use]
    pub fn similar_face_value(
        &self,
        face: FaceId,
        kind: SimilarFace,
        matrix: &Mat4,
    ) -> SimilarValue {
        let space = Space::new(matrix);
        let corners: Vec<Vec3> = self
            .face_verts(face)
            .map(|vert| space.point(self.position(vert)))
            .collect();
        match kind {
            SimilarFace::Material => SimilarValue::Material(self.face_material(face)),
            SimilarFace::Area => SimilarValue::Scalar(
                newell_from(corners[corners.len() - 1], corners.iter().copied()).length() * 0.5,
            ),
            SimilarFace::Sides => SimilarValue::Count(self.face_len(face) as i64),
            SimilarFace::Perimeter => SimilarValue::Scalar(
                (0..corners.len())
                    .map(|index| corners[index].distance(corners[(index + 1) % corners.len()]))
                    .sum(),
            ),
            SimilarFace::Normal => {
                SimilarValue::Direction(normalize(space.normal(self.face_normal(face))))
            }
            SimilarFace::Coplanar => {
                let normal = normalize(space.normal(self.face_normal(face)));
                SimilarValue::Plane(normal.extend(-normal.dot(corners[0])))
            }
            SimilarFace::Smooth => SimilarValue::Flag(self.is_face_smooth(face)),
            SimilarFace::Freestyle => {
                SimilarValue::Flag(self.bool_attribute(FREESTYLE_FACE, Domain::Face, face.index()))
            }
        }
    }

    #[must_use]
    pub fn selected_similar_vert_values(
        &self,
        kind: SimilarVert,
        matrix: &Mat4,
    ) -> Vec<SimilarValue> {
        self.verts()
            .filter(|&vert| self.is_vert_selected(vert))
            .map(|vert| self.similar_vert_value(vert, kind, matrix))
            .collect()
    }

    #[must_use]
    pub fn selected_similar_edge_values(
        &self,
        kind: SimilarEdge,
        matrix: &Mat4,
    ) -> Vec<SimilarValue> {
        self.edges()
            .filter(|&edge| self.is_edge_selected(edge))
            .filter_map(|edge| self.similar_edge_value(edge, kind, matrix))
            .collect()
    }

    #[must_use]
    pub fn selected_similar_face_values(
        &self,
        kind: SimilarFace,
        matrix: &Mat4,
    ) -> Vec<SimilarValue> {
        self.faces()
            .filter(|&face| self.is_face_selected(face))
            .map(|face| self.similar_face_value(face, kind, matrix))
            .collect()
    }

    pub fn select_similar_verts(
        &mut self,
        kind: SimilarVert,
        compare: Compare,
        threshold: f32,
    ) -> bool {
        let references = self.selected_similar_vert_values(kind, &Mat4::IDENTITY);
        self.select_similar_verts_matching(kind, compare, threshold, &references, &Mat4::IDENTITY)
    }

    pub fn select_similar_verts_matching(
        &mut self,
        kind: SimilarVert,
        compare: Compare,
        threshold: f32,
        references: &[SimilarValue],
        matrix: &Mat4,
    ) -> bool {
        if references.is_empty() {
            return false;
        }
        let radians = threshold * PI + f32::EPSILON;
        let counts = counts(references);
        let normals = directions(references);
        let creases = scalars(references);
        let groups: BTreeSet<String> = references
            .iter()
            .filter_map(|value| match value {
                SimilarValue::Groups(names) => Some(names.iter().cloned()),
                _ => None,
            })
            .flatten()
            .collect();
        if kind == SimilarVert::Groups && groups.is_empty() {
            return false;
        }
        let candidates: Vec<VertId> = self
            .verts()
            .filter(|&vert| !self.is_vert_selected(vert) && !self.is_vert_hidden(vert))
            .collect();
        let mut changed = false;
        for vert in candidates {
            let hit = match self.similar_vert_value(vert, kind, matrix) {
                SimilarValue::Count(count) => counts
                    .iter()
                    .any(|&reference| compare_int(count - reference, compare)),
                SimilarValue::Direction(normal) => {
                    nearest_angle(&normals, normal).is_some_and(|angle| angle <= radians)
                }
                SimilarValue::Groups(names) => names.iter().any(|name| groups.contains(name)),
                SimilarValue::Scalar(value) => {
                    compare_float_set(&creases, value, threshold, compare)
                }
                _ => false,
            };
            if hit {
                self.select_vert(vert, true);
                changed = true;
            }
        }
        if changed {
            self.select_mode_flush();
        }
        changed
    }

    pub fn select_similar_edges(
        &mut self,
        kind: SimilarEdge,
        compare: Compare,
        threshold: f32,
    ) -> bool {
        if !self.edges().any(|edge| self.is_edge_selected(edge)) {
            return false;
        }
        let references = self.selected_similar_edge_values(kind, &Mat4::IDENTITY);
        self.select_similar_edges_matching(kind, compare, threshold, &references, &Mat4::IDENTITY)
    }

    pub fn select_similar_edges_matching(
        &mut self,
        kind: SimilarEdge,
        compare: Compare,
        threshold: f32,
        references: &[SimilarValue],
        matrix: &Mat4,
    ) -> bool {
        let radians = threshold * PI + f32::EPSILON;
        let flags = flags(references);
        let values = scalars(references);
        let directions = directions(references);
        let counts = counts(references);
        let flag_kind = matches!(
            kind,
            SimilarEdge::Seam | SimilarEdge::Sharp | SimilarEdge::Freestyle
        );
        let select_all = flag_kind && flags.len() == 2;
        let candidates: Vec<EdgeId> = self
            .edges()
            .filter(|&edge| !self.is_edge_selected(edge) && !self.is_edge_hidden(edge))
            .collect();
        let mut changed = false;
        for edge in candidates {
            let hit = select_all
                || match (kind, self.similar_edge_value(edge, kind, matrix)) {
                    (_, Some(SimilarValue::Count(count))) => counts
                        .iter()
                        .any(|&reference| compare_int(count - reference, compare)),
                    (_, Some(SimilarValue::Direction(direction))) => {
                        nearest_angle(&directions, direction).is_some_and(|angle| angle <= radians)
                    }
                    (SimilarEdge::FaceAngle, Some(SimilarValue::Scalar(value))) => {
                        self.edge_face_count_at_most(edge, 3) == 2
                            && compare_float_set(&values, value, threshold, Compare::Equal)
                    }
                    (_, Some(SimilarValue::Scalar(value))) => {
                        compare_float_set(&values, value, threshold, compare)
                    }
                    (_, Some(SimilarValue::Flag(flag))) => flags.contains(&flag),
                    _ => false,
                };
            if hit {
                self.select_edge(edge, true);
                changed = true;
            }
        }
        if changed {
            self.select_mode_flush();
        }
        changed
    }

    pub fn select_similar_faces(
        &mut self,
        kind: SimilarFace,
        compare: Compare,
        threshold: f32,
    ) -> bool {
        let references = self.selected_similar_face_values(kind, &Mat4::IDENTITY);
        self.select_similar_faces_matching(kind, compare, threshold, &references, &Mat4::IDENTITY)
    }

    pub fn select_similar_faces_matching(
        &mut self,
        kind: SimilarFace,
        compare: Compare,
        threshold: f32,
        references: &[SimilarValue],
        matrix: &Mat4,
    ) -> bool {
        if references.is_empty() {
            return false;
        }
        let radians = threshold * PI;
        let flags = flags(references);
        let values = scalars(references);
        let normals = directions(references);
        let sides = counts(references);
        let planes: Vec<Vec4> = references
            .iter()
            .filter_map(|value| match value {
                SimilarValue::Plane(plane) => Some(*plane),
                _ => None,
            })
            .collect();
        let materials: BTreeSet<i32> = references
            .iter()
            .filter_map(|value| match value {
                SimilarValue::Material(material) => Some(*material),
                _ => None,
            })
            .collect();
        let flag_kind = matches!(kind, SimilarFace::Smooth | SimilarFace::Freestyle);
        let select_all = flag_kind && flags.len() == 2;
        let candidates: Vec<FaceId> = self
            .faces()
            .filter(|&face| !self.is_face_selected(face) && !self.is_face_hidden(face))
            .collect();
        let mut changed = false;
        for face in candidates {
            let hit = select_all
                || match self.similar_face_value(face, kind, matrix) {
                    SimilarValue::Count(count) => sides
                        .iter()
                        .any(|&reference| compare_int(count - reference, compare)),
                    SimilarValue::Material(material) => materials.contains(&material),
                    SimilarValue::Scalar(value) => {
                        compare_float_set(&values, value, threshold, compare)
                    }
                    SimilarValue::Direction(normal) => {
                        nearest_angle(&normals, normal).is_some_and(|angle| angle <= radians)
                    }
                    SimilarValue::Plane(plane) => planes
                        .iter()
                        .min_by(|a, b| {
                            a.distance_squared(plane)
                                .total_cmp(&b.distance_squared(plane))
                        })
                        .is_some_and(|&nearest| {
                            nearest.distance(plane) <= threshold
                                && (plane.w - nearest.w).abs() <= threshold
                                && angle_vectors(plane.truncate(), nearest.truncate()) <= radians
                        }),
                    SimilarValue::Flag(flag) => flags.contains(&flag),
                    SimilarValue::Groups(_) => false,
                };
            if hit {
                self.select_face(face, true);
                changed = true;
            }
        }
        if changed {
            self.select_mode_flush();
        }
        changed
    }

    pub fn select_similar_region(&mut self) -> SimilarRegions {
        let selected = self
            .faces()
            .filter(|&face| self.is_face_selected(face))
            .count();
        if selected < 2 {
            return SimilarRegions::TooFewFaces;
        }
        let groups = self.calc_face_groups(|face| self.is_face_selected(face));
        let mut found = 0;
        for group in groups {
            for region in self.region_match(&group) {
                for face in region {
                    self.select_face(face, true);
                }
                found += 1;
            }
        }
        SimilarRegions::Found(found)
    }
}
