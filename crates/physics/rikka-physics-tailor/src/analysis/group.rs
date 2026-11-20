use rikka_model_core::Bounds;

use super::attach::attach;
use super::features::MeshFeatures;
use super::parts::{majority, parts};
use super::suggest::{Silhouette, stands, suggest_hair, suggest_sheet};
use crate::asset::PieceKind;
use crate::cloth_settings::Role;
use crate::geometry::PointGrid;
use crate::preset::Preset;

const MIN_EXTENT: f32 = 0.025;
const HAIR_COVER: f32 = 0.02;
const HAIR_COVERED: f32 = 0.5;

#[derive(Clone, Debug)]
pub(crate) struct Candidate {
    pub(crate) vertices: Vec<u32>,
    pub(crate) kind: PieceKind,
    pub(crate) preset: Preset,
    pub(crate) title: String,
}

struct Grouping<'s, 'a, 'm> {
    features: &'s MeshFeatures<'a, 'm>,
    silhouette: Silhouette,
    surfaces: Vec<u32>,
    hair: PointGrid,
}

pub(crate) fn group(
    features: &MeshFeatures<'_, '_>,
    roles: &mut [Role],
    strands: &[bool],
) -> Vec<Candidate> {
    let parts = parts(features, roles, strands);
    let grouping = Grouping::new(features, roles, strands);
    let mut candidates: Vec<Candidate> = Vec::new();
    for garment in attach(features, &parts, roles) {
        let members: Vec<u32> = sorted(
            garment
                .iter()
                .flat_map(|part| parts[*part].members.iter().copied()),
        );
        let context: Vec<u32> = sorted(
            garment
                .iter()
                .filter(|part| !parts[**part].is_attachment)
                .flat_map(|part| parts[*part].members.iter().copied()),
        );
        let mostly_hair = context
            .iter()
            .filter(|vertex| strands[**vertex as usize])
            .count()
            * 2
            > context.len();
        if !grouping.is_cloth(&context, &members, roles, mostly_hair) {
            for vertex in &members {
                roles[*vertex as usize] = Role::Skinned;
            }
            continue;
        }
        let candidate = grouping.candidate(&context, members, roles, mostly_hair);
        match candidates.iter_mut().find(|other| {
            other.kind == candidate.kind
                && other.preset == candidate.preset
                && other.title == candidate.title
        }) {
            Some(other) => other.vertices.extend(candidate.vertices),
            None => candidates.push(candidate),
        }
    }
    for candidate in &mut candidates {
        candidate.vertices.sort_unstable();
    }
    candidates
}

impl<'s, 'a, 'm> Grouping<'s, 'a, 'm> {
    fn new(
        features: &'s MeshFeatures<'a, 'm>,
        roles: &[Role],
        strands: &[bool],
    ) -> Grouping<'s, 'a, 'm> {
        let mut surfaces = vec![u32::MAX; features.figure.vertices.len()];
        for triangle in &features.figure.triangles {
            for corner in triangle.corners {
                if surfaces[corner as usize] == u32::MAX {
                    surfaces[corner as usize] = triangle.surface;
                }
            }
        }
        let hair = PointGrid::new(
            (0..features.figure.vertices.len() as u32)
                .filter(|vertex| {
                    strands[*vertex as usize] || features.class[*vertex as usize].is_hair()
                })
                .map(|vertex| (vertex, features.position(vertex))),
            HAIR_COVER * 2.0,
        );
        Grouping {
            features,
            silhouette: Silhouette::measure(features, roles),
            surfaces,
            hair,
        }
    }

    fn is_cloth(
        &self,
        context: &[u32],
        members: &[u32],
        roles: &[Role],
        mostly_hair: bool,
    ) -> bool {
        let free = |vertices: &[u32]| -> Vec<u32> {
            vertices
                .iter()
                .copied()
                .filter(|vertex| roles[*vertex as usize] == Role::Free)
                .collect()
        };
        let moving = free(context);
        let extent = Bounds::from_points(
            free(members)
                .iter()
                .map(|vertex| self.features.position(*vertex)),
        )
        .longest();
        !moving.is_empty() && extent >= MIN_EXTENT && (mostly_hair || !self.under_hair(&moving))
    }

    fn under_hair(&self, free: &[u32]) -> bool {
        let covered = free
            .iter()
            .filter(|vertex| {
                let at = self.features.position(**vertex);
                self.hair
                    .near(at, HAIR_COVER)
                    .any(|hair| self.features.position(hair).distance(at) <= HAIR_COVER)
            })
            .count();
        covered as f32 >= HAIR_COVERED * free.len() as f32
    }

    fn candidate(
        &self,
        context: &[u32],
        members: Vec<u32>,
        roles: &[Role],
        mostly_hair: bool,
    ) -> Candidate {
        let features = self.features;
        let preset = match mostly_hair {
            true => suggest_hair(features, context, roles),
            false if stands(features, context, roles) => Preset::Accessory,
            false => suggest_sheet(features, &self.silhouette, context, roles),
        };
        let title = majority(
            context
                .iter()
                .map(|vertex| self.surfaces[*vertex as usize])
                .filter(|surface| *surface != u32::MAX),
        )
        .map(|surface| {
            features.figure.surfaces[surface as usize]
                .title()
                .to_string()
        })
        .unwrap_or_default();
        Candidate {
            vertices: members,
            kind: if mostly_hair {
                PieceKind::Strands
            } else {
                preset.kind()
            },
            preset,
            title,
        }
    }
}

fn sorted(vertices: impl Iterator<Item = u32>) -> Vec<u32> {
    let mut vertices: Vec<u32> = vertices.collect();
    vertices.sort_unstable();
    vertices
}
