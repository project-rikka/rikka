use std::sync::Arc;

use animate_cast::FigureId;
use animate_fluid::EmitterId;
use rikka_model::Bounds;
use rikka_model::glam::{Mat4, Vec3};

use crate::Stage;
use crate::fluid::Placing;
use crate::fluid::aiming::{Aiming, Joint};
use crate::fluid::liquids::Liquids;
use crate::render::Render;
use crate::scene::Scene;

impl Stage {
    pub fn set_emitters(&mut self, liquids: Arc<Liquids>) {
        if *self.scene.liquids == *liquids {
            return;
        }
        let scene = self.change();
        if let Some(owner) = scene.drafts.pour.owner()
            && liquids.emitter(owner).is_none()
        {
            scene.drafts.pour.discard();
        }
        scene.liquids = liquids;
        self.resource(None);
    }

    pub fn pour_into(&mut self, emitter: Option<EmitterId>) {
        if self.scene.pouring != emitter {
            self.change().pouring = emitter;
            self.outputs.aiming = None;
        }
    }

    pub fn aiming(&self) -> Option<&Aiming> {
        self.outputs.aiming.as_ref()
    }

    pub fn placing(&self, figure: Option<(FigureId, Bounds)>) -> Placing {
        let Some((id, rest)) = figure else {
            return Placing::for_view(&self.scene.free);
        };
        Placing::for_figure(self.bounds_of(id).unwrap_or(rest))
    }
}

impl Render {
    fn locate(&self, figure: &str, bone: &str) -> Option<Mat4> {
        self.actors
            .iter()
            .find(|actor| actor.name == figure)?
            .bone_world(bone)
    }

    pub(crate) fn aim_emitter(
        &self,
        scene: &Scene,
        view_projection: Mat4,
        time: f32,
        running: bool,
    ) -> Option<Aiming> {
        let emitter = scene.liquids.emitter(scene.pouring?)?;
        let gravity: Vec3 = scene.liquids.settings.gravity.vector();
        let mut aiming = Aiming::new(
            emitter.id,
            scene.emitter_state(emitter, time),
            gravity,
            view_projection,
            |figure, bone| self.locate(figure, bone),
        );
        if !running {
            aiming.joints = self.joints();
        }
        Some(aiming)
    }

    fn joints(&self) -> Vec<Joint> {
        self.actors
            .iter()
            .filter(|actor| actor.visible)
            .flat_map(|actor| {
                actor
                    .skeleton
                    .bones()
                    .iter()
                    .filter(|bone| bone.shown)
                    .map(|bone| Joint {
                        figure: actor.name.clone(),
                        bone: bone.name.clone(),
                        world: actor.instance.world(bone.node).w_axis.truncate(),
                    })
            })
            .collect()
    }
}
