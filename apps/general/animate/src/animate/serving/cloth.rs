use animate_inspector::ClothEditing;
use std::sync::Arc;

use animate_assistant::PieceRequest;
use animate_cast::{FigureId, Garment};
use animate_cloth::{Knob, kind_id, preset_id};
use rikka_physics_tailor::{AnalyzedPiece, ClothAnalysis, ClothTuning, Quality};
use rikka_tool::Reply;
use serde_json::{Map, Value as Json, json};

use super::{ServeError, ToolResult, round};
use crate::animate::Animate;

impl Animate {
    fn analyzed_cloth(
        &mut self,
        id: FigureId,
        figure: &str,
    ) -> Result<Arc<ClothAnalysis>, ServeError> {
        self.analyze_now(id)
            .ok_or_else(|| ServeError::FigureRemoved {
                name: figure.to_owned(),
            })?
            .map_err(ServeError::Cloth)
    }

    pub(super) fn read_cloth(&mut self, figure: &str) -> ToolResult {
        let id = self.figure_called(figure)?;
        let analysis = self.analyzed_cloth(id, figure)?;
        let Some(cast_figure) = self.cast.figure(id) else {
            return Err(ServeError::FigureRemoved {
                name: figure.to_owned(),
            });
        };
        let cloth_settings = cast_figure.cloth.cloth_settings();
        let pieces: Vec<Json> = analysis
            .pieces()
            .iter()
            .map(|piece| {
                let garment = Garment::new(cloth_settings, piece);
                let tuning = described(&garment.tuning);
                json!({
                    "piece": piece.name,
                    "kind": kind_id(piece.kind),
                    "preset": preset_id(garment.preset),
                    "suggested": preset_id(piece.suggested_preset),
                    "enabled": garment.enabled,
                    "layer": garment.layer.map_or_else(|| json!("auto"), |layer| json!(layer + 1)),
                    "feel": tuning,
                    "detailed": garment.detailed,
                })
            })
            .collect();
        let quality = match self.cloth_quality() {
            Quality::Low => "low",
            Quality::Medium => "medium",
            Quality::High => "high",
        };
        Ok(Reply::json(&json!({
            "figure": figure,
            "quality": quality,
            "feel": described(&cloth_settings.tuning()),
            "pieces": pieces,
        })))
    }

    pub(super) fn set_cloth(
        &mut self,
        figure: &str,
        whole: &[(Knob, f32)],
        orders: &[PieceRequest],
    ) -> ToolResult {
        let id = self.figure_called(figure)?;
        let analysis = self.analyzed_cloth(id, figure)?;
        let chosen = orders
            .iter()
            .map(|request| {
                analysis
                    .pieces()
                    .iter()
                    .find(|piece| piece.name == request.piece)
                    .cloned()
                    .map(|piece| (piece, request.clone()))
                    .ok_or_else(|| unknown(figure, &request.piece, analysis.pieces()))
            })
            .collect::<Result<Vec<(AnalyzedPiece, PieceRequest)>, ServeError>>()?;
        self.restyle(id, whole, chosen);
        self.read_cloth(figure)
    }

    fn restyle(
        &mut self,
        id: FigureId,
        whole: &[(Knob, f32)],
        chosen: Vec<(AnalyzedPiece, PieceRequest)>,
    ) {
        let Some(figure) = self.cast.figure(id) else {
            return;
        };
        let mut cloth_settings = (**figure.cloth.cloth_settings()).clone();
        let mut tuning = cloth_settings.tuning();
        for (knob, value) in whole {
            knob.set(&mut tuning, *value);
        }
        cloth_settings.set_tuning(tuning);
        for (piece, request) in chosen {
            let settings = cloth_settings.settings_mut(&piece);
            if request.reset {
                settings.tuning = ClothTuning::NEUTRAL;
                settings.style = None;
            }
            if let Some(preset) = request.preset {
                settings.preset = Some(preset);
                settings.style = None;
            }
            if let Some(enabled) = request.enabled {
                settings.enabled = enabled;
            }
            for (knob, value) in request.feel {
                knob.set(&mut settings.tuning, value);
                settings.style = None;
            }
        }
        self.apply_cloth_settings(id, Arc::new(cloth_settings));
    }
}

fn described(tuning: &ClothTuning) -> Map<String, Json> {
    Knob::ALL
        .into_iter()
        .map(|knob| (knob.id().to_owned(), round(knob.get(tuning))))
        .collect()
}

fn unknown(figure: &str, piece: &str, pieces: &[AnalyzedPiece]) -> ServeError {
    match pieces.is_empty() {
        true => ServeError::NoPieces {
            figure: figure.to_owned(),
        },
        false => ServeError::UnknownPiece {
            figure: figure.to_owned(),
            piece: piece.to_owned(),
            pieces: pieces
                .iter()
                .map(|piece| format!("\"{}\"", piece.name))
                .collect::<Vec<String>>()
                .join(", "),
        },
    }
}
