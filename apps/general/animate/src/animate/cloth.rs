use std::sync::Arc;

use animate_cast::{AnalysisResult, BodyFit, Figure, FigureId, Garment, Subject, fit_physique};
use animate_cloth::{Field, Knob, Reading};
use animate_fit::Physique;
use animate_inspector::{ClothEditing, Dial, Pane};
use rikka_app::prelude::*;
use rikka_physics_tailor::{
    AnalyzedPiece, ClothSettings, ClothTuning, PieceKey, PieceSettings, Preset, Quality,
};

use crate::animate::Animate;
use crate::animate::edit::{Edit, Pair};
use crate::wording;

const CLOTH_ANALYSIS_THREAD: &str = "rikka-animate-analysis";

impl Animate {
    pub(crate) fn paints_cloth(&self) -> bool {
        self.controls.pane == Pane::Physics && self.controls.painting
    }

    pub(super) fn fit_cloth(&self) {
        let body_fit = match self.subject {
            Some(Subject::Figure(id)) if self.paints_cloth() => Some(id),
            _ => None,
        };
        self.stage.lock().fit_cloth(body_fit);
    }

    pub(super) fn load_cloth(&mut self, id: FigureId) {
        let Some(figure) = self.cast.figure(id) else {
            return;
        };
        let cloth = &figure.cloth;
        let mut stage = self.stage.lock();
        stage.set_cloth_settings(
            id,
            Arc::clone(cloth.cloth_settings()),
            Arc::clone(cloth.mirror()),
        );
        stage.show_analysis(id, cloth.analysis().cloned());
    }

    pub(crate) fn apply_cloth_settings(&mut self, id: FigureId, after: Arc<ClothSettings>) {
        let Some(figure) = self.cast.figure(id) else {
            return;
        };
        let pair = Pair::new(
            Arc::clone(figure.cloth.cloth_settings()),
            Arc::clone(&after),
        );
        if pair.is_empty() {
            return;
        }
        self.set_cloth_settings(id, after);
        self.record(Subject::Figure(id), Edit::Cloth(pair));
    }

    fn edit_cloth_settings(&mut self, id: FigureId, change: impl FnOnce(&mut ClothSettings)) {
        let Some(figure) = self.cast.figure(id) else {
            return;
        };
        let mut cloth_settings = (**figure.cloth.cloth_settings()).clone();
        change(&mut cloth_settings);
        self.apply_cloth_settings(id, Arc::new(cloth_settings));
    }

    pub(crate) fn edit_piece_settings(
        &mut self,
        key: PieceKey,
        change: impl FnOnce(&mut PieceSettings, &AnalyzedPiece),
    ) {
        let Some(figure) = self.chosen_figure() else {
            return;
        };
        let id = figure.id;
        let Some(piece) = figure.cloth.piece(key).cloned() else {
            return;
        };
        self.controls.cloth_draft = None;
        self.edit_cloth_settings(id, |cloth_settings| {
            change(cloth_settings.settings_mut(&piece), &piece);
        });
    }

    pub(super) fn set_cloth_settings(&mut self, id: FigureId, cloth_settings: Arc<ClothSettings>) {
        let Some(figure) = self.cast.figure_mut(id) else {
            return;
        };
        figure.cloth.set_settings(Arc::clone(&cloth_settings));
        let mirror = Arc::clone(figure.cloth.mirror());
        let model = Arc::clone(&figure.model);
        let anatomy = Arc::clone(&figure.anatomy);
        let body = figure.body.clone();
        let edits = figure.cloth.edits();
        self.stage
            .lock()
            .set_cloth_settings(id, Arc::clone(&cloth_settings), mirror);
        let sender = self.sender.clone();
        let spawned = std::thread::Builder::new()
            .name(CLOTH_ANALYSIS_THREAD.to_owned())
            .spawn(move || {
                let analysis = animate_cast::analyze(&model, &anatomy, &cloth_settings);
                let body_fit =
                    fit_physique(body.as_deref(), &model, &anatomy, analysis.as_deref().ok());
                let _ = sender.send(move |animate: &mut Animate| {
                    animate.finish_analysis(id, edits, analysis, body_fit);
                });
            });
        if let Err(error) = spawned {
            self.notice = Some(Label::from(error.to_string()));
        }
    }

    pub(crate) fn finish_analysis(
        &mut self,
        id: FigureId,
        edits: u64,
        analysis: AnalysisResult,
        body_fit: BodyFit,
    ) {
        let Some(figure) = self.cast.figure_mut(id) else {
            return;
        };
        if !figure.cloth.finish_analysis(edits, analysis) {
            skip_view();
            return;
        }
        let analysis = figure.cloth.analysis().cloned();
        self.stage.lock().show_analysis(id, analysis);
        self.apply_body_fit(id, body_fit);
    }

    pub(crate) fn analyze_now(&mut self, id: FigureId) -> Option<AnalysisResult> {
        let figure = self.cast.figure_mut(id)?;
        let model = Arc::clone(&figure.model);
        let anatomy = Arc::clone(&figure.anatomy);
        let body = figure.body.clone();
        let reanalyzed = figure.cloth.is_analyzing();
        let analysis = figure.cloth.analyze_now(&model, &anatomy);
        self.stage
            .lock()
            .show_analysis(id, analysis.as_ref().ok().cloned());
        if reanalyzed {
            let body_fit =
                fit_physique(body.as_deref(), &model, &anatomy, analysis.as_deref().ok());
            self.apply_body_fit(id, body_fit);
        }
        Some(analysis)
    }

    fn apply_body_fit(&mut self, id: FigureId, body_fit: BodyFit) {
        let physique = match body_fit {
            Ok(physique) => physique,
            Err(error) => {
                if let Some(figure) = self.cast.figure(id) {
                    self.notice = Some(wording::body_fit(&figure.name, &error));
                }
                None
            }
        };
        self.refit(id, physique);
    }

    fn refit(&mut self, id: FigureId, physique: Option<Arc<Physique>>) {
        let eye = self.eye();
        let Some(figure) = self.cast.figure_mut(id) else {
            return;
        };
        if figure.physique.as_deref() == physique.as_deref() {
            return;
        }
        figure.physique = physique;
        let Some(playback) = &figure.playback else {
            return;
        };
        let playback = Arc::new(playback.rebaked(&figure.take, figure.performer(), &eye));
        figure.playback = Some(Arc::clone(&playback));
        self.stage.lock().pose(id, Some(playback), Some(0));
    }

    fn retune_figure(&mut self, change: impl FnOnce(&mut ClothTuning)) {
        self.edit_chosen_cloth_settings(|cloth_settings| {
            let mut tuning = cloth_settings.tuning();
            change(&mut tuning);
            cloth_settings.set_tuning(tuning);
        });
    }

    fn edit_chosen_cloth_settings(&mut self, change: impl FnOnce(&mut ClothSettings)) {
        let Some(id) = self.chosen_figure().map(|figure| figure.id) else {
            return;
        };
        self.controls.cloth_draft = None;
        self.edit_cloth_settings(id, change);
    }
}

impl ClothEditing for Animate {
    fn can_save_cloth(&self) -> bool {
        self.chosen_figure()
            .is_some_and(|figure| figure.cloth.path().is_some() && figure.cloth.is_dirty())
    }
    fn choose_piece(&mut self, key: PieceKey) {
        self.controls.piece = Some(key);
        self.controls.cloth_draft = None;
    }
    fn chosen_figure(&self) -> Option<&Figure> {
        let Some(Subject::Figure(id)) = self.subject else {
            return None;
        };
        self.cast.figure(id)
    }
    fn chosen_piece(&self) -> Option<&AnalyzedPiece> {
        self.chosen_figure()?.cloth.piece(self.controls.piece?)
    }
    fn cloth_draft(&self, dial: Dial) -> Option<f32> {
        self.controls
            .cloth_draft
            .filter(|(candidate, _)| *candidate == dial)
            .map(|(_, value)| value)
    }
    fn cloth_quality(&self) -> Quality {
        self.preferences.cloth_quality()
    }
    fn draft_cloth(&mut self, dial: Dial, value: f32) {
        self.controls.cloth_draft = Some((dial, value));
    }
    fn figure_tuning(&self) -> Option<ClothTuning> {
        Some(self.chosen_figure()?.cloth.cloth_settings().tuning())
    }
    fn garment(&self, piece: &AnalyzedPiece) -> Option<Garment> {
        let figure = self.chosen_figure()?;
        Some(Garment::new(figure.cloth.cloth_settings(), piece))
    }
    fn redetect(&mut self) {
        self.edit_chosen_cloth_settings(ClothSettings::clear_paint);
    }
    fn reset_figure_tuning(&mut self) {
        self.retune_figure(|tuning| *tuning = ClothTuning::NEUTRAL);
    }
    fn reset_piece(&mut self, key: PieceKey) {
        self.edit_piece_settings(key, |settings, _| {
            settings.tuning = ClothTuning::NEUTRAL;
            settings.style = None;
        });
    }
    fn save_cloth(&mut self) {
        let Some(Subject::Figure(id)) = self.subject else {
            return;
        };
        let Some(figure) = self.cast.figure_mut(id) else {
            return;
        };
        if let Err(error) = figure.cloth.save() {
            self.notice = Some(Label::from(animate_ui::error_chain(&error)));
        }
    }
    fn set_cloth_quality(&mut self, quality: Quality) {
        self.preferences.keep_cloth_quality(quality);
        self.stage.lock().set_cloth_quality(quality);
    }
    fn set_field(&mut self, key: PieceKey, field: Field, reading: Reading) {
        self.edit_piece_settings(key, |settings, piece| {
            let preset = settings.preset.unwrap_or(piece.suggested_preset);
            let mut style = settings.resolved_style(preset);
            field.write(&mut style, reading);
            settings.style = Some(style);
        });
    }
    fn set_figure_knob(&mut self, knob: Knob, value: f32) {
        self.retune_figure(|tuning| knob.set(tuning, value));
    }
    fn set_knob(&mut self, key: PieceKey, knob: Knob, value: f32) {
        self.edit_piece_settings(key, |settings, _| {
            knob.set(&mut settings.tuning, value);
            settings.style = None;
        });
    }
    fn set_layer(&mut self, key: PieceKey, layer: Option<u8>) {
        self.edit_piece_settings(key, |settings, _| settings.layer = layer);
    }
    fn set_piece_enabled(&mut self, key: PieceKey, enabled: bool) {
        self.edit_piece_settings(key, |settings, _| settings.enabled = enabled);
    }
    fn set_preset(&mut self, key: PieceKey, preset: Preset) {
        self.edit_piece_settings(key, |settings, _| {
            settings.preset = Some(preset);
            settings.style = None;
        });
    }
    fn show_painting(&mut self, open: bool) {
        self.controls.painting = open;
        self.fit_cloth();
    }
    fn show_pane(&mut self, pane: Pane) {
        self.controls.pane = pane;
        self.controls.cloth_draft = None;
        self.fit_cloth();
    }
}
