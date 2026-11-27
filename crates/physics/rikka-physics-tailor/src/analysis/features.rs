use glam::Vec3;
use rikka_model_core::{Bounds, HumanBone, NodeId};

use super::class::{Class, classify};
use super::hints::carried;
use crate::PARALLEL;
use crate::figure::Figure;
use crate::geometry::{Graph, PointGrid, Shell, Union, groups};

const SAME: f32 = 2e-5;
const DUPLICATES_GRAIN: usize = 8192;
const REACH: f32 = 0.25;
pub(crate) const FREE_SHARE: f32 = 0.25;

#[derive(Clone, Debug)]
pub(crate) struct Region {
    pub(crate) class: Class,
    pub(crate) vertices: Vec<u32>,
    pub(crate) bounds: Bounds,
    pub(crate) hinted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Head {
    pub(crate) node: NodeId,
    pub(crate) centre: Vec3,
    pub(crate) forward: Vec3,
}

#[derive(Debug)]
pub(crate) struct MeshFeatures<'a, 'm> {
    pub(crate) figure: &'a Figure<'m>,
    pub(crate) class: Vec<Class>,
    pub(crate) region: Vec<u32>,
    pub(crate) regions: Vec<Region>,
    pub(crate) carried: Vec<bool>,
    pub(crate) share: Vec<f32>,
    pub(crate) skin: Shell,
    duplicate_pairs: Vec<(u32, u32)>,
    pub(crate) head: Option<Head>,
    offsets: Vec<f32>,
}

impl<'a, 'm> MeshFeatures<'a, 'm> {
    pub(crate) fn new(figure: &'a Figure<'m>) -> MeshFeatures<'a, 'm> {
        let carried = carried(figure);
        let surface_class = classify(figure, &carried);
        let class = vertex_classes(figure, &surface_class);
        let duplicate_pairs = duplicates(figure, &class);
        let (region, count) = regions(figure, &class, &surface_class, &duplicate_pairs);
        let share: Vec<f32> = figure
            .vertices
            .iter()
            .map(|vertex| vertex.share(|node| carried[node.index()]))
            .collect();
        let regions: Vec<Region> = groups(&region, count)
            .into_iter()
            .map(|vertices| Region {
                class: class[vertices[0] as usize],
                bounds: Bounds::from_points(
                    vertices
                        .iter()
                        .map(|v| figure.vertices[*v as usize].position),
                ),
                hinted: vertices.iter().any(|v| share[*v as usize] >= FREE_SHARE),
                vertices,
            })
            .collect();
        let skin = skin_shell(figure, &class, &regions);
        let head = head(figure, &class);
        MeshFeatures {
            figure,
            class,
            region,
            regions,
            carried,
            share,
            skin,
            duplicate_pairs,
            head,
            offsets: vec![f32::NAN; figure.vertices.len()],
        }
    }

    pub(crate) fn links(&self, keep: impl Fn(u32) -> bool) -> Graph {
        let joined = |a: u32, b: u32| {
            keep(a) && keep(b) && self.region[a as usize] == self.region[b as usize]
        };
        let mut links: Vec<(u32, u32, f32)> = Vec::new();
        for triangle in &self.figure.triangles {
            for side in 0..3 {
                let (a, b) = (triangle.corners[side], triangle.corners[(side + 1) % 3]);
                if joined(a, b) {
                    links.push((a, b, self.position(a).distance(self.position(b))));
                }
            }
        }
        for &(a, b) in &self.duplicate_pairs {
            if joined(a, b) {
                links.push((a, b, 0.0));
            }
        }
        Graph::new(self.figure.vertices.len(), links)
    }

    pub(crate) fn offset(&mut self, vertex: u32) -> f32 {
        let cached = self.offsets[vertex as usize];
        if !cached.is_nan() {
            return cached;
        }
        let offset = match self.class[vertex as usize] {
            Class::Skin => 0.0,
            _ => self
                .skin
                .signed(self.figure.vertices[vertex as usize].position, REACH)
                .map_or(REACH, |signed| signed.distance),
        };
        self.offsets[vertex as usize] = offset;
        offset
    }

    pub(crate) fn hair_exists(&self) -> bool {
        self.regions.iter().any(|region| region.class.is_hair())
    }

    pub(crate) fn hair_hinted(&self) -> bool {
        self.regions
            .iter()
            .any(|region| region.class.is_hair() && region.hinted)
    }

    pub(crate) fn garments_hinted(&self) -> bool {
        self.regions
            .iter()
            .any(|region| matches!(region.class, Class::Garment | Class::Unknown) && region.hinted)
    }

    pub(crate) fn position(&self, vertex: u32) -> Vec3 {
        self.figure.vertices[vertex as usize].position
    }
}

fn vertex_classes(figure: &Figure<'_>, surface_class: &[Class]) -> Vec<Class> {
    let mut counts: Vec<[u32; Class::RANKS]> = vec![[0; Class::RANKS]; figure.vertices.len()];
    let mut first: Vec<[Class; Class::RANKS]> =
        vec![[Class::Unknown; Class::RANKS]; figure.vertices.len()];
    for triangle in &figure.triangles {
        let class = surface_class[triangle.surface as usize];
        let rank = class.rank() as usize;
        for corner in triangle.corners {
            let counted = &mut counts[corner as usize];
            if counted[rank] == 0 {
                first[corner as usize][rank] = class;
            }
            counted[rank] += 1;
        }
    }
    counts
        .iter()
        .zip(&first)
        .map(|(counted, classes)| {
            let rank = (0..Class::RANKS)
                .max_by_key(|rank| counted[*rank])
                .filter(|rank| counted[*rank] > 0);
            rank.map_or(Class::Unknown, |rank| classes[rank])
        })
        .collect()
}

fn duplicates(figure: &Figure<'_>, class: &[Class]) -> Vec<(u32, u32)> {
    let grid = PointGrid::new(
        figure
            .vertices
            .iter()
            .enumerate()
            .map(|(index, vertex)| (index as u32, vertex.position)),
        SAME * 4.0,
    );
    PARALLEL
        .grain(DUPLICATES_GRAIN)
        .concat_chunks(figure.vertices.len(), |range| {
            let mut pairs = Vec::new();
            for index in range {
                let vertex = &figure.vertices[index];
                let index = index as u32;
                for other in grid.near(vertex.position, SAME) {
                    let other_vertex = &figure.vertices[other as usize];
                    if other > index
                        && other_vertex.part == vertex.part
                        && class[other as usize] == class[index as usize]
                        && other_vertex.position.distance(vertex.position) <= SAME
                    {
                        pairs.push((index, other));
                    }
                }
            }
            pairs
        })
}

fn regions(
    figure: &Figure<'_>,
    class: &[Class],
    surface_class: &[Class],
    duplicate_pairs: &[(u32, u32)],
) -> (Vec<u32>, usize) {
    let mut union = Union::new(figure.vertices.len());
    for triangle in &figure.triangles {
        let own = surface_class[triangle.surface as usize];
        let corners: Vec<u32> = triangle
            .corners
            .iter()
            .copied()
            .filter(|corner| class[*corner as usize] == own)
            .collect();
        for pair in corners.windows(2) {
            union.join(pair[0], pair[1]);
        }
    }
    for &(a, b) in duplicate_pairs {
        union.join(a, b);
    }
    union.labels()
}

fn skin_shell(figure: &Figure<'_>, class: &[Class], regions: &[Region]) -> Shell {
    let positions: Vec<Vec3> = figure
        .vertices
        .iter()
        .map(|vertex| vertex.position)
        .collect();
    let skin: Vec<[u32; 3]> = figure
        .triangles
        .iter()
        .filter(|triangle| {
            triangle
                .corners
                .iter()
                .all(|corner| class[*corner as usize] == Class::Skin)
        })
        .map(|triangle| triangle.corners)
        .collect();
    if !skin.is_empty() {
        return Shell::new(&positions, &skin);
    }
    let widest = regions
        .iter()
        .enumerate()
        .filter(|(_, region)| matches!(region.class, Class::Unknown | Class::Garment))
        .max_by_key(|(index, region)| {
            let mut bones: Vec<NodeId> = region
                .vertices
                .iter()
                .map(|vertex| figure.vertices[*vertex as usize].strongest())
                .collect();
            bones.sort_unstable();
            bones.dedup();
            (bones.len(), std::cmp::Reverse(*index))
        })
        .map(|(_, region)| region);
    let Some(widest) = widest else {
        return Shell::new(&positions, &[]);
    };
    let mut included = vec![false; figure.vertices.len()];
    for vertex in &widest.vertices {
        included[*vertex as usize] = true;
    }
    let triangles: Vec<[u32; 3]> = figure
        .triangles
        .iter()
        .filter(|triangle| {
            triangle
                .corners
                .iter()
                .all(|corner| included[*corner as usize])
        })
        .map(|triangle| triangle.corners)
        .collect();
    Shell::new(&positions, &triangles)
}

fn head(figure: &Figure<'_>, class: &[Class]) -> Option<Head> {
    let node = figure.anatomy.node(HumanBone::Head)?;
    let under = &figure.skeleton.under_head;
    let around = |include: fn(Class) -> bool| {
        Bounds::from_points(
            figure
                .vertices
                .iter()
                .zip(class)
                .filter(|(vertex, class)| under[vertex.strongest().index()] && include(**class))
                .map(|(vertex, _)| vertex.position),
        )
    };
    let face = around(|class| matches!(class, Class::Skin | Class::Detail));
    let around = if face.is_empty() {
        around(|class| !class.is_hair())
    } else {
        face
    };
    let centre = if around.is_empty() {
        figure.skeleton.origins[node.index()] + Vec3::Y * 0.08
    } else {
        around.center()
    };
    Some(Head {
        node,
        centre,
        forward: figure.anatomy.forward(),
    })
}
