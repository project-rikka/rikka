use glam::Vec3;
use rikka_humanoid::Anatomy;
use rikka_model_core::{Model, NodeId};

use crate::analysis::{AnalyzedPiece, ClothAnalysis, MeshFeatures};
use crate::asset::{ClothAsset, Piece, PieceKind};
use crate::body::{colliders, skin};
use crate::budget::Budget;
use crate::cloth_settings::{ClothSettings, Role};
use crate::constraints::{ConstraintContext, build_constraints};
use crate::error::Result;
use crate::figure::Figure;
use crate::layers::layer;
use crate::proxy::Proxy;
use crate::sheet::Sheet;
use crate::strands::{BandWidths, strands};
use crate::underlay::underlay;

const GROWTH: f32 = 1.2;
const ROUNDS: usize = 16;
const SETTLED_REACH: f32 = 0.02;

impl ClothAsset {
    pub fn build(
        model: &Model,
        anatomy: &Anatomy,
        cloth_settings: &ClothSettings,
        budget: Budget,
    ) -> Result<ClothAsset> {
        let budget = budget.sanitized();
        let figure = Figure::new(model, anatomy)?;
        let mut features = MeshFeatures::new(&figure);
        let analysis = ClothAnalysis::from_features(&mut features, cloth_settings);
        let head = features.head;
        let mut roles = analysis.roles().to_vec();
        for piece in analysis.pieces().iter().filter(|piece| !piece.enabled) {
            for vertex in &piece.vertices {
                roles[*vertex as usize] = Role::Skinned;
            }
        }
        let skin = skin(&figure, &roles);
        let spreads = analysis.spreads();
        let enabled: Vec<&AnalyzedPiece> = analysis
            .pieces()
            .iter()
            .filter(|piece| piece.enabled)
            .collect();
        let mut sheets: Vec<Option<Sheet>> = enabled
            .iter()
            .map(|piece| match piece.kind {
                PieceKind::Sheet => Sheet::new(&figure, &piece.vertices, &roles),
                PieceKind::Strands => None,
            })
            .collect();
        let mut spacings = vec![budget.spacing; enabled.len()];
        let mut built: Vec<Option<f32>> = vec![None; enabled.len()];
        let mut ribbons: Vec<Option<Proxy>> = enabled.iter().map(|_| None).collect();
        for _ in 0..ROUNDS {
            for (at, piece) in enabled.iter().enumerate() {
                if built[at] == Some(spacings[at]) {
                    continue;
                }
                built[at] = Some(spacings[at]);
                match piece.kind {
                    PieceKind::Sheet => {
                        if let Some(sheet) = &mut sheets[at] {
                            sheet.reduce(spacings[at], &skin.shell);
                        }
                    }
                    PieceKind::Strands => {
                        ribbons[at] = strands(
                            &figure,
                            head,
                            &piece.vertices,
                            &roles,
                            spreads,
                            BandWidths::from_band(spacings[at]),
                        );
                    }
                }
            }
            let counts: Vec<usize> = sheets
                .iter()
                .zip(&ribbons)
                .map(|(sheet, ribbon)| {
                    sheet.as_ref().map_or(0, Sheet::count) + ribbon.as_ref().map_or(0, Proxy::len)
                })
                .collect();
            let limit = budget.piece_limit(&counts);
            let crowded: Vec<usize> = (0..counts.len()).filter(|&at| counts[at] > limit).collect();
            if crowded.is_empty() {
                break;
            }
            for at in crowded {
                spacings[at] *= GROWTH;
            }
        }
        for (at, piece) in enabled.iter().enumerate() {
            if piece.kind == PieceKind::Strands && ribbons[at].is_none() {
                sheets[at] = Sheet::new(&figure, &piece.vertices, &roles);
                if let Some(sheet) = &mut sheets[at] {
                    sheet.reduce(spacings[at], &skin.shell);
                }
            }
        }
        let context = ConstraintContext {
            figure: &figure,
            body: &skin.shell,
            body_normals: &skin.normals,
            body_triangles: &skin.body.triangles,
            root: model.roots.first().copied().unwrap_or(NodeId(0)),
        };
        let mut pieces = Vec::new();
        let mut overrides = Vec::new();
        for ((piece, sheet), ribbon) in enabled.iter().zip(sheets).zip(ribbons) {
            let proxy = match ribbon {
                Some(ribbon) => Some(ribbon),
                None => sheet.and_then(|sheet| sheet.proxy(&figure, spreads)),
            };
            let Some(proxy) = proxy else {
                continue;
            };
            let blank = Piece {
                key: piece.key,
                name: piece.name.clone(),
                kind: piece.kind,
                preset: piece.preset,
                style: piece.style.clone(),
                layer: 0,
                particles: Vec::new(),
                edges: Vec::new(),
                bends: Vec::new(),
                tethers: Vec::new(),
                triangles: Vec::new(),
                strand_nodes: Vec::new(),
                bindings: Vec::new(),
            };
            if let Some(constrained) = build_constraints(&context, proxy, &piece.style, blank) {
                pieces.push(constrained);
                overrides.push(piece.layer);
            }
        }
        layer(&mut pieces, &overrides);
        underlay(&mut pieces);
        let samples: Vec<(Vec3, f32)> = pieces
            .iter()
            .flat_map(|piece| piece.particles.iter())
            .filter(|particle| !particle.pinned && particle.reach >= SETTLED_REACH)
            .map(|particle| (particle.rest, particle.radius))
            .collect();
        let mut body = skin.body;
        body.colliders = colliders(&figure, &roles, &samples);
        Ok(ClothAsset {
            rest_worlds: figure.rest_worlds.clone(),
            pieces,
            body,
        })
    }

    #[must_use]
    pub fn particle_count(&self) -> usize {
        self.pieces.iter().map(|piece| piece.particles.len()).sum()
    }
}
