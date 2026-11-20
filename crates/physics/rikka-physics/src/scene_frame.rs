use std::sync::Arc;

use rikka_model_core::NodeId;
use rikka_physics_cloth::Drape;
use rikka_physics_fluid::{BodyId, Surface, Wetting};
use rikka_renderer_3d::{FluidLayer, FluidMaterial, Frame, Wetness};

use crate::draping;
use crate::layout::{FigureKey, Layout};

#[derive(Clone, Debug)]
pub struct SceneFrame {
    surface: Surface,
    wet: Arc<[(FigureKey, Wetting)]>,
    drapes: Arc<[(FigureKey, Vec<Drape>)]>,
}

impl SceneFrame {
    pub(crate) fn new(
        surface: Surface,
        layout: &Layout,
        bodies: &[(usize, BodyId)],
        wetness: impl Fn(BodyId) -> Option<Wetting>,
        drapes: Vec<(FigureKey, Vec<Drape>)>,
    ) -> SceneFrame {
        SceneFrame {
            surface,
            wet: bodies
                .iter()
                .filter_map(|(figure, body)| Some((layout.key_of(*figure)?, wetness(*body)?)))
                .collect(),
            drapes: drapes.into(),
        }
    }

    #[must_use]
    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    #[must_use]
    pub fn cloth(&self, figure: FigureKey) -> &[Drape] {
        self.drapes
            .iter()
            .find(|(key, _)| *key == figure)
            .map(|(_, drapes)| drapes.as_slice())
            .unwrap_or_default()
    }

    pub fn draw_fluid(&self, frame: &mut Frame, materials: &[FluidMaterial]) {
        let surface = &self.surface;
        if surface.particle_count == 0 && surface.spray_count == 0 {
            return;
        }
        frame.fluid(
            FluidLayer::new(
                &surface.particles,
                surface.particle_count,
                surface.radius,
                &surface.ready,
            )
            .spray(&surface.spray, surface.spray_count)
            .materials(materials.iter().copied()),
        );
    }

    pub fn wetness(&self, frame: &mut Frame, figure: FigureKey) -> Option<Wetness> {
        let (_, wetting) = self.wet.iter().find(|(key, _)| *key == figure)?;
        Some(frame.wetness(&wetting.buffer, &wetting.ready))
    }

    pub fn drapes(
        &self,
        frame: &mut Frame,
        figure: FigureKey,
    ) -> Vec<(NodeId, rikka_renderer_3d::Drape)> {
        draping::to_frame(frame, self.cloth(figure))
    }
}
