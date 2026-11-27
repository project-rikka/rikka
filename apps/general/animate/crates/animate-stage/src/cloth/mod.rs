use std::sync::Arc;

use animate_cast::FigureId;
use animate_cloth::{Mirror, Surface};
use rikka_model::glam::{Mat4, Vec3};
use rikka_physics_tailor::{ClothAnalysis, ClothSettings};

use crate::Stage;
use crate::render::Render;
use crate::scene::Scene;

#[derive(Clone, Debug)]
pub struct FigureCloth {
    pub cloth_settings: Arc<ClothSettings>,
    pub analysis: Option<Arc<ClothAnalysis>>,
    analyzed: Option<Arc<ClothSettings>>,
    pub mirror: Arc<Mirror>,
    pub revision: u64,
}

impl FigureCloth {
    pub(crate) fn current_analysis(&self) -> Option<&Arc<ClothAnalysis>> {
        self.analyzed
            .as_ref()
            .filter(|analyzed| Arc::ptr_eq(analyzed, &self.cloth_settings))
            .and(self.analysis.as_ref())
    }
}

#[derive(Debug)]
pub struct PaintSurface {
    pub figure: FigureId,
    pub view_projection: Mat4,
    pub eye: Vec3,
    pub surface: Surface,
}

impl Stage {
    pub fn figure_cloth_for(&self, figure: FigureId) -> Option<&FigureCloth> {
        self.scene.figure_cloth.get(&figure)
    }

    pub fn set_cloth_settings(
        &mut self,
        figure: FigureId,
        cloth_settings: Arc<ClothSettings>,
        mirror: Arc<Mirror>,
    ) {
        let revision = match self.scene.figure_cloth.get(&figure) {
            Some(figure_cloth) if Arc::ptr_eq(&figure_cloth.cloth_settings, &cloth_settings) => {
                return;
            }
            Some(figure_cloth) => figure_cloth.revision + 1,
            None => 1,
        };
        let scene = self.change();
        let (analysis, analyzed) = scene
            .figure_cloth
            .remove(&figure)
            .map_or((None, None), |figure_cloth| {
                (figure_cloth.analysis, figure_cloth.analyzed)
            });
        scene.figure_cloth.insert(
            figure,
            FigureCloth {
                cloth_settings,
                analysis,
                analyzed,
                mirror,
                revision,
            },
        );
        self.watchers.repaint();
    }

    pub fn show_analysis(&mut self, figure: FigureId, analysis: Option<Arc<ClothAnalysis>>) {
        if !self.scene.figure_cloth.contains_key(&figure) {
            return;
        }
        if let Some(figure_cloth) = self.change().figure_cloth.get_mut(&figure) {
            figure_cloth.analyzed = analysis
                .as_ref()
                .map(|_| Arc::clone(&figure_cloth.cloth_settings));
            figure_cloth.analysis = analysis;
        }
        self.watchers.repaint();
    }

    pub fn fit_cloth(&mut self, figure: Option<FigureId>) {
        if self.scene.paint_figure != figure {
            self.change().paint_figure = figure;
            self.outputs.paint_surface = None;
        }
    }

    pub fn paint_surface(&self) -> Option<&Arc<PaintSurface>> {
        self.outputs.paint_surface.as_ref()
    }
}

impl Render {
    pub(crate) fn settle_cloth_assets(&mut self, scene: &Scene) {
        self.request_cloth_assets(scene);
        self.simulation.wait_for_cloth_assets();
    }

    pub(crate) fn paint_surface_at(
        &self,
        scene: &Scene,
        view_projection: Mat4,
        eye: Vec3,
    ) -> Option<Arc<PaintSurface>> {
        let figure = scene.paint_figure?;
        let actor = self
            .actors
            .iter()
            .find(|actor| actor.id == figure && actor.visible)?;
        Some(Arc::new(PaintSurface {
            figure,
            view_projection,
            eye,
            surface: Surface::from_instance(&actor.instance),
        }))
    }
}
