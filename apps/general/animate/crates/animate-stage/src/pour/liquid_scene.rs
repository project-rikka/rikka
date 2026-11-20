use std::sync::Arc;

use animate_body::{Body, Faces, MorphWeights};
use animate_cast::{FigureId, Skeleton, TakePlayback};
use animate_fluid::{Emitter, Shape};
use animate_wind::{Wind, WindMotion};
use rikka_model::humanoid::Anatomy;
use rikka_model::mmd::FRAMES_PER_SECOND;
use rikka_model::{Instance, Model};
use rikka_model_gpu::HeadRest;
use rikka_physics::{
    EmitterState, FigureKey, FigureMotion, FluidBuilder, Layout, LiquidEmitter, Mix, Region,
};
use rikka_physics_tailor::Grab;
use rikka_renderer_3d::FluidMaterial;

use super::emitter_state;
use super::grid::{GRID, step_of_frame};
use crate::Liquids;
use crate::actor::perform;
use crate::holding::{self, Performer, Pinches};
use crate::physics::ClothScene;

const FLOOR: f32 = 0.0;

#[derive(Clone, Debug)]
pub(crate) struct CastMember {
    pub id: FigureId,
    pub name: String,
    pub model: Arc<Model>,
    pub anatomy: Arc<Anatomy>,
    pub skeleton: Arc<Skeleton>,
    pub body: Option<Arc<Body>>,
    pub faces: Arc<Faces>,
    pub head: Option<HeadRest>,
    pub pinches: Pinches,
    pub playback: Option<Arc<TakePlayback>>,
}

impl CastMember {
    fn plays_like(&self, other: &CastMember) -> bool {
        self.id == other.id && self.name == other.name && Arc::ptr_eq(&self.model, &other.model)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LiquidScene {
    figures: Vec<CastMember>,
    liquids: Arc<Liquids>,
    wind: Arc<WindMotion>,
    cloth_scene: Option<ClothScene>,
    layout: Layout,
    physics_emitters: Vec<LiquidEmitter>,
    materials: Vec<FluidMaterial>,
}

impl Default for LiquidScene {
    fn default() -> Self {
        Self::new(Vec::new(), Arc::default(), Arc::default(), None)
    }
}

impl LiquidScene {
    pub(crate) fn new(
        figures: Vec<CastMember>,
        liquids: Arc<Liquids>,
        wind: Arc<WindMotion>,
        cloth_scene: Option<ClothScene>,
    ) -> Self {
        let settings = liquids.settings;
        let fluid = FluidBuilder::default()
            .radius(settings.radius())
            .capacity(settings.capacity)
            .gravity(settings.gravity.vector())
            .floor(FLOOR);
        let base = Layout::new(fluid)
            .drying(settings.drying_rate())
            .step_rate(GRID);
        let base = match &cloth_scene {
            Some(cloth_scene) => base.cloth(cloth_scene.cloth),
            None => base,
        };
        let layout = figures.iter().fold(base, |layout, member| {
            let model = Arc::clone(&member.model);
            match cloth_scene
                .as_ref()
                .and_then(|cloth_scene| cloth_scene.cloth_asset(member.id))
            {
                Some(cloth_asset) => layout.figure_with_cloth(
                    FigureKey(member.id.number()),
                    model,
                    Arc::clone(cloth_asset),
                ),
                None => layout.figure(FigureKey(member.id.number()), model),
            }
        });
        let physics_emitters: Vec<LiquidEmitter> = liquids
            .emitters
            .iter()
            .map(|emitter| LiquidEmitter {
                id: emitter.id.0,
                liquid: emitter.liquid.liquid(),
                region: region(emitter, &figures),
            })
            .collect();
        Self {
            figures,
            liquids,
            wind,
            cloth_scene,
            layout,
            materials: Mix::from_emitters(&physics_emitters).materials(),
            physics_emitters,
        }
    }

    pub(crate) fn materials(&self) -> &[FluidMaterial] {
        &self.materials
    }

    pub(crate) fn changed_from(&self, before: &LiquidScene) -> Option<u32> {
        let recast = self.figures.len() != before.figures.len()
            || self
                .figures
                .iter()
                .zip(&before.figures)
                .any(|(now, then)| !now.plays_like(then));
        let cloth_changed = match (&self.cloth_scene, &before.cloth_scene) {
            (Some(now), Some(then)) => !now.same_as(then),
            (now, then) => now.is_some() != then.is_some(),
        };
        if recast || cloth_changed || self.liquids.settings != before.liquids.settings {
            return Some(0);
        }
        let (now, then) = (&self.liquids.emitters, &before.liquids.emitters);
        if now.len() != then.len() || now.iter().zip(then).any(|(one, two)| one.id != two.id) {
            return Some(0);
        }
        let blown = self.wind.changed_from(&before.wind).map(step_of_frame);
        now.iter()
            .zip(then)
            .filter(|(one, two)| !Arc::ptr_eq(one, two))
            .filter_map(|(one, two)| Emitter::changed_from(one, two))
            .map(step_of_frame)
            .chain(blown)
            .min()
    }
}

impl rikka_physics::SceneSource for LiquidScene {
    fn layout(&self) -> &Layout {
        &self.layout
    }

    fn emitters(&self) -> &[LiquidEmitter] {
        &self.physics_emitters
    }

    fn wind(&self, time: f32) -> Wind {
        self.wind.sample(time * FRAMES_PER_SECOND)
    }

    fn pose(&self, figure: usize, time: f32, instance: &mut Instance) -> FigureMotion {
        let Some(member) = self.figures.get(figure) else {
            return FigureMotion::Resting;
        };
        let unregistered = MorphWeights::default();
        perform(
            member.playback.as_deref(),
            member.body.as_deref(),
            &unregistered,
            instance,
            time,
        )
        .motion
    }

    fn emitter_states(&self, step: u32, figures: &[Instance]) -> Vec<EmitterState> {
        let locate = |name: &str, bone: &str| {
            let index = self.figures.iter().position(|member| member.name == name)?;
            self.figures[index]
                .skeleton
                .world_of(figures.get(index)?, bone)
        };
        let gravity = self.liquids.settings.gravity.vector();
        self.liquids
            .emitters
            .iter()
            .map(|emitter| emitter_state::emitter_state(emitter, step, gravity, locate))
            .collect()
    }

    fn grabs(&self, time: f32, figures: &[Instance]) -> Vec<(usize, Grab)> {
        let performers: Vec<Performer<'_>> = self
            .figures
            .iter()
            .zip(figures)
            .map(|(member, instance)| Performer {
                key: member.id.number(),
                name: &member.name,
                instance,
                pinches: member.pinches,
                holds: member
                    .playback
                    .as_ref()
                    .map(|playback| playback.holds().as_ref()),
                cloth_asset: self
                    .cloth_scene
                    .as_ref()
                    .and_then(|cloth_scene| cloth_scene.cloth_asset(member.id)),
            })
            .collect();
        holding::active_grabs(time, &performers)
            .into_iter()
            .map(|active_grabs| (active_grabs.owner, active_grabs.grab))
            .collect()
    }
}

fn region(emitter: &Emitter, figures: &[CastMember]) -> Option<Region> {
    let Shape::Surface { figure, materials } = &emitter.shape else {
        return None;
    };
    Some(Region {
        figure: figures.iter().position(|member| member.name == *figure)?,
        materials: materials.clone(),
    })
}
