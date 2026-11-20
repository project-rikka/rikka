use rikka_model_core::Instance;
use rikka_physics_liquid::Liquid;
use rikka_physics_tailor::{Grab, Wind};

use crate::emitter::EmitterState;
use crate::layout::Layout;

pub trait SceneSource: Send + Sync + 'static {
    fn layout(&self) -> &Layout;

    fn emitters(&self) -> &[LiquidEmitter];

    fn pose(&self, figure: usize, time: f32, instance: &mut Instance) -> FigureMotion;

    fn emitter_states(&self, step: u32, figures: &[Instance]) -> Vec<EmitterState>;

    fn wind(&self, time: f32) -> Wind;

    fn grabs(&self, _time: f32, _figures: &[Instance]) -> Vec<(usize, Grab)> {
        Vec::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FigureMotion {
    Moving,
    Resting,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiquidEmitter {
    pub id: u32,
    pub liquid: Liquid,
    pub region: Option<Region>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub figure: usize,
    pub materials: Vec<u32>,
}

pub(crate) fn has_emitters(source: &dyn SceneSource) -> bool {
    !source.emitters().is_empty()
}
