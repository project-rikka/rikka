use std::path::{Path, PathBuf};
use std::sync::Arc;

use animate_cloth::{Mirror, Surface};
use rikka_model::Model;
use rikka_model::humanoid::Anatomy;
use rikka_physics_tailor::{
    self, AnalyzedPiece, ClothAnalysis, ClothSettings, ClothTuning, PieceKey, Preset, Style,
};

pub type AnalysisResult = Result<Arc<ClothAnalysis>, Arc<rikka_physics_tailor::Error>>;

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("the figure was not read from a file")]
    NoSourceFile,
    #[error("cannot write the cloth settings")]
    Write(#[source] rikka_physics_tailor::Error),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Garment {
    pub preset: Preset,
    pub tuning: ClothTuning,
    pub style: Style,
    pub detailed: bool,
    pub enabled: bool,
    pub layer: Option<u8>,
}

impl Garment {
    pub fn new(cloth_settings: &ClothSettings, piece: &AnalyzedPiece) -> Self {
        let settings = cloth_settings.settings(piece);
        let preset = settings
            .and_then(|settings| settings.preset)
            .unwrap_or(piece.suggested_preset);
        Self {
            preset,
            tuning: settings.map_or(ClothTuning::NEUTRAL, |settings| settings.tuning),
            style: settings.map_or_else(
                || preset.style(),
                |settings| settings.resolved_style(preset),
            ),
            detailed: settings.is_some_and(|settings| settings.style.is_some()),
            enabled: settings.is_none_or(|settings| settings.enabled),
            layer: settings.and_then(|settings| settings.layer),
        }
    }
}

#[derive(Debug)]
pub struct Cloth {
    path: Option<PathBuf>,
    cloth_settings: Arc<ClothSettings>,
    saved: Arc<ClothSettings>,
    analysis: Option<AnalysisResult>,
    edits: u64,
    analyzed: u64,
    mirror: Arc<Mirror>,
}

impl Cloth {
    pub fn new(model: &Model, path: Option<PathBuf>, cloth_settings: ClothSettings) -> Self {
        let cloth_settings = Arc::new(cloth_settings);
        Self {
            path,
            analysis: None,
            saved: Arc::clone(&cloth_settings),
            cloth_settings,
            edits: 1,
            analyzed: 0,
            mirror: Arc::new(Mirror::from_surface(&Surface::rest(model))),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn cloth_settings(&self) -> &Arc<ClothSettings> {
        &self.cloth_settings
    }

    pub fn mirror(&self) -> &Arc<Mirror> {
        &self.mirror
    }

    pub fn analysis(&self) -> Option<&Arc<ClothAnalysis>> {
        self.analysis.as_ref()?.as_ref().ok()
    }

    pub fn error(&self) -> Option<&rikka_physics_tailor::Error> {
        self.analysis.as_ref()?.as_ref().err().map(Arc::as_ref)
    }

    pub fn edits(&self) -> u64 {
        self.edits
    }

    pub fn is_analyzing(&self) -> bool {
        self.analyzed != self.edits
    }

    pub fn is_dirty(&self) -> bool {
        !Arc::ptr_eq(&self.cloth_settings, &self.saved) && *self.cloth_settings != *self.saved
    }

    pub fn piece(&self, key: PieceKey) -> Option<&AnalyzedPiece> {
        self.analysis()?
            .pieces()
            .iter()
            .find(|piece| piece.key == key)
    }

    pub fn set_settings(&mut self, cloth_settings: Arc<ClothSettings>) {
        self.cloth_settings = cloth_settings;
        self.edits += 1;
    }

    pub fn finish_analysis(&mut self, edits: u64, analysis: AnalysisResult) -> bool {
        if edits != self.edits {
            return false;
        }
        self.analysis = Some(analysis);
        self.analyzed = edits;
        true
    }

    pub fn analyze_now(&mut self, model: &Model, anatomy: &Anatomy) -> AnalysisResult {
        if let (false, Some(analysis)) = (self.is_analyzing(), &self.analysis) {
            return analysis.clone();
        }
        let edits = self.edits;
        let analysis = analyze(model, anatomy, &self.cloth_settings);
        self.finish_analysis(edits, analysis.clone());
        analysis
    }

    pub fn save(&mut self) -> Result<(), SaveError> {
        let Some(path) = &self.path else {
            return Err(SaveError::NoSourceFile);
        };
        let mut saved = (*self.cloth_settings).clone();
        if let Some(analysis) = self.analysis().filter(|_| !self.is_analyzing()) {
            saved.sync_pieces(analysis.pieces());
        }
        saved.save_for_model(path).map_err(SaveError::Write)?;
        self.saved = Arc::clone(&self.cloth_settings);
        Ok(())
    }
}

pub fn analyze(model: &Model, anatomy: &Anatomy, cloth_settings: &ClothSettings) -> AnalysisResult {
    ClothAnalysis::new(model, anatomy, cloth_settings)
        .map(Arc::new)
        .map_err(Arc::new)
}
