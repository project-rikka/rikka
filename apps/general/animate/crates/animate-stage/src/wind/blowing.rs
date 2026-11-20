use std::sync::Arc;

use animate_wind::WindMotion;
use rikka_model::glam::{Mat4, Vec3};

use crate::Stage;
use crate::render::Render;
use crate::scene::Scene;
use crate::wind::vane::Vane;

impl Stage {
    pub fn set_wind(&mut self, wind: Arc<WindMotion>) {
        if *self.scene.wind != *wind {
            self.change().wind = wind;
            self.resource(None);
        }
    }

    pub fn show_wind(&mut self, visible: bool) {
        if self.scene.blowing != visible {
            self.change().blowing = visible;
            self.outputs.vane = None;
        }
    }

    pub fn vane(&self) -> Option<&Vane> {
        self.outputs.vane.as_ref()
    }
}

impl Render {
    pub(crate) fn project_wind(
        &self,
        scene: &Scene,
        view_projection: Mat4,
        looking_at: Vec3,
        time: f32,
    ) -> Option<Vane> {
        scene.blowing.then(|| Vane {
            view_projection,
            anchor: Vec3::new(looking_at.x, 0.0, looking_at.z),
            wind: scene.wind_at(time),
        })
    }
}
