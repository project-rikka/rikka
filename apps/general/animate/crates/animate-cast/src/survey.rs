use std::sync::Arc;

use animate_body::Body;
use rikka_model::Model;
use rikka_model::humanoid::Anatomy;
use rikka_physics_tailor::ClothSettings;

use crate::cast::{Figure, FigureId};
use crate::cloth::{AnalysisResult, analyze};
use crate::physique::{BodyFit, fit_physique};

#[derive(Debug)]
pub struct Survey {
    figure: FigureId,
    edits: u64,
    model: Arc<Model>,
    anatomy: Arc<Anatomy>,
    body: Option<Arc<Body>>,
    cloth_settings: Arc<ClothSettings>,
}

#[derive(Debug)]
pub struct Surveyed {
    pub figure: FigureId,
    pub edits: u64,
    pub analysis: AnalysisResult,
    pub body_fit: BodyFit,
}

impl Survey {
    pub fn of(figure: &Figure) -> Survey {
        Survey {
            figure: figure.id,
            edits: figure.cloth.edits(),
            model: Arc::clone(&figure.model),
            anatomy: Arc::clone(&figure.anatomy),
            body: figure.body.clone(),
            cloth_settings: Arc::clone(figure.cloth.cloth_settings()),
        }
    }

    pub fn run(self) -> Surveyed {
        let analysis = analyze(&self.model, &self.anatomy, &self.cloth_settings);
        let body_fit = fit_physique(
            self.body.as_deref(),
            &self.model,
            &self.anatomy,
            analysis.as_deref().ok(),
        );
        Surveyed {
            figure: self.figure,
            edits: self.edits,
            analysis,
            body_fit,
        }
    }
}
