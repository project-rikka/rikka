mod attach;
mod class;
mod decide;
mod features;
mod group;
mod hard;
mod hints;
mod inner;
mod parts;
mod suggest;

use rikka_humanoid::Anatomy;
use rikka_model_core::{Model, NodeId};

pub(crate) use features::{Head, MeshFeatures};

use crate::asset::{PieceKey, PieceKind};
use crate::cloth_settings::{ClothSettings, Role};
use crate::error::Result;
use crate::figure::Figure;
use crate::key::VertexKey;
use crate::preset::Preset;
use crate::style::Style;
use decide::decide;
use group::group;

#[derive(Clone, Debug, PartialEq)]
pub struct AnalyzedPiece {
    pub key: PieceKey,
    pub name: String,
    pub kind: PieceKind,
    pub preset: Preset,
    pub suggested_kind: PieceKind,
    pub suggested_preset: Preset,
    pub enabled: bool,
    pub layer: Option<u8>,
    pub style: Style,
    members: Vec<VertexKey>,
    pub(crate) vertices: Vec<u32>,
}

impl AnalyzedPiece {
    #[must_use]
    pub fn members(&self) -> &[VertexKey] {
        &self.members
    }

    #[must_use]
    pub fn contains(&self, vertex: VertexKey) -> bool {
        self.members.binary_search(&vertex).is_ok()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClothAnalysis {
    parts: Vec<(NodeId, u32, u32)>,
    roles: Vec<Role>,
    pieces: Vec<AnalyzedPiece>,
}

impl ClothAnalysis {
    pub fn new(
        model: &Model,
        anatomy: &Anatomy,
        cloth_settings: &ClothSettings,
    ) -> Result<ClothAnalysis> {
        let figure = Figure::new(model, anatomy)?;
        let mut features = MeshFeatures::new(&figure);
        Ok(ClothAnalysis::from_features(&mut features, cloth_settings))
    }

    pub(crate) fn from_features(
        features: &mut MeshFeatures<'_, '_>,
        cloth_settings: &ClothSettings,
    ) -> ClothAnalysis {
        let figure = features.figure;
        let auto = decide(features);
        let mut roles = auto.roles;
        let mut strands = auto.strands;
        for (key, paint) in cloth_settings.paint_entries() {
            let (Some(role), Some(vertex)) = (paint.role, figure.index(key)) else {
                continue;
            };
            roles[vertex as usize] = role;
            if matches!(role, Role::Free | Role::Pinned) && !strands[vertex as usize] {
                strands[vertex as usize] = features.class[vertex as usize].is_hair();
            }
        }
        let candidates = group(features, &mut roles, &strands);
        let mut pieces: Vec<AnalyzedPiece> = candidates
            .into_iter()
            .map(|candidate| {
                let members: Vec<VertexKey> = candidate
                    .vertices
                    .iter()
                    .map(|vertex| figure.key(*vertex))
                    .collect();
                let mut sorted = members.clone();
                sorted.sort_unstable();
                AnalyzedPiece {
                    key: PieceKey::from(sorted[0]),
                    name: candidate.title.clone(),
                    kind: candidate.kind,
                    preset: candidate.preset,
                    suggested_kind: candidate.kind,
                    suggested_preset: candidate.preset,
                    enabled: true,
                    layer: None,
                    style: candidate.preset.style(),
                    members: sorted,
                    vertices: candidate.vertices,
                }
            })
            .collect();
        pieces.sort_by_key(|piece| piece.key);
        name_pieces(&mut pieces);
        let matched = cloth_settings.matched(&pieces);
        for (piece, index) in pieces.iter_mut().zip(matched) {
            let Some(settings) = index.map(|index| &cloth_settings.pieces()[index]) else {
                piece.style = cloth_settings.tuning().apply(&piece.style);
                continue;
            };
            piece.enabled = settings.enabled;
            piece.preset = settings.preset.unwrap_or(piece.preset);
            piece.kind = settings.kind.unwrap_or(match settings.preset {
                Some(preset) if piece.suggested_kind == PieceKind::Sheet => preset.kind(),
                _ => piece.kind,
            });
            piece.layer = settings.layer;
            piece.style = cloth_settings
                .tuning()
                .apply(&settings.resolved_style(piece.preset));
        }
        ClothAnalysis {
            parts: figure
                .parts
                .iter()
                .map(|part| (part.node, part.first, part.len))
                .collect(),
            roles,
            pieces,
        }
    }

    #[must_use]
    pub fn pieces(&self) -> &[AnalyzedPiece] {
        &self.pieces
    }

    #[must_use]
    pub fn role(&self, vertex: VertexKey) -> Option<Role> {
        self.index(vertex).map(|index| self.roles[index as usize])
    }

    pub fn vertices(&self) -> impl Iterator<Item = (VertexKey, Role)> + '_ {
        self.parts.iter().flat_map(move |(node, first, len)| {
            (0..*len).map(move |vertex| {
                (
                    VertexKey::new(*node, vertex),
                    self.roles[(first + vertex) as usize],
                )
            })
        })
    }

    pub(crate) fn roles(&self) -> &[Role] {
        &self.roles
    }

    fn index(&self, vertex: VertexKey) -> Option<u32> {
        self.parts
            .iter()
            .find(|(node, _, len)| *node == vertex.node && vertex.vertex < *len)
            .map(|(_, first, _)| first + vertex.vertex)
    }
}

fn name_pieces(pieces: &mut [AnalyzedPiece]) {
    let titles: Vec<String> = pieces.iter().map(|piece| piece.name.clone()).collect();
    for (index, piece) in pieces.iter_mut().enumerate() {
        let shared = titles
            .iter()
            .enumerate()
            .any(|(other, title)| other != index && *title == piece.name);
        piece.name = match (piece.name.is_empty(), shared) {
            (true, _) => piece.preset.name().to_string(),
            (false, true) => format!("{} ({})", piece.name, piece.preset.name()),
            (false, false) => piece.name.clone(),
        };
    }
    let names: Vec<String> = pieces.iter().map(|piece| piece.name.clone()).collect();
    for (index, piece) in pieces.iter_mut().enumerate() {
        let before = names[..index]
            .iter()
            .filter(|name| **name == names[index])
            .count();
        let after = names[index + 1..].iter().any(|name| *name == names[index]);
        if before > 0 || after {
            piece.name = format!("{} {}", names[index], before + 1);
        }
    }
}
