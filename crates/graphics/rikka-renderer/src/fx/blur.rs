use std::sync::LazyLock;

use rikka_types::Size;

use crate::fx::effect::{BackdropPass, BackdropShader, PassExtent, PassList, PostEffect};

static DOWN_SHADER: LazyLock<BackdropShader> =
    LazyLock::new(|| BackdropShader::builtin(shaders::BLUR_DOWN_FRAG, "rikka.fx.blur_down"));
static UP_SHADER: LazyLock<BackdropShader> =
    LazyLock::new(|| BackdropShader::builtin(shaders::BLUR_UP_FRAG, "rikka.fx.blur_up"));

const DOWN_SPREAD: f32 = 1.0 / 2.0;
const DOWN_FLOOR: f32 = 1.0 / 3.0;
const UP_SPREAD: f32 = 1.0 / 12.0;
const UP_FLOOR: f32 = 1.0 / 12.0;

const MAX_OFFSET: f32 = 2.0;

const MAX_STEPS: u32 = 6;

#[derive(Clone, Copy, Debug)]
#[must_use]
pub struct Blur {
    sigma: f32,
}

impl Blur {
    pub fn new(sigma: f32) -> Self {
        Self {
            sigma: sigma.max(0.0),
        }
    }

    pub fn with_radius(radius: f32) -> Self {
        Self::new(radius / 3.0)
    }

    #[must_use]
    pub fn sigma(&self) -> f32 {
        self.sigma
    }

    fn chain(&self) -> Option<(u32, f32)> {
        if self.sigma <= 0.0 {
            return None;
        }
        let variance = self.sigma * self.sigma;
        for steps in 1..MAX_STEPS {
            let (spread, floor) = accumulated(steps);
            let offset = ((variance - floor) / spread).max(0.0).sqrt();
            if offset <= MAX_OFFSET {
                return Some((steps, offset));
            }
        }
        let (spread, floor) = accumulated(MAX_STEPS);
        Some((MAX_STEPS, ((variance - floor) / spread).max(0.0).sqrt()))
    }
}

fn accumulated(steps: u32) -> (f32, f32) {
    let mut spread = 0.0;
    let mut floor = 0.0;
    for step in 1..=steps {
        let above = 4f32.powi(step as i32 - 1);
        spread += DOWN_SPREAD * above;
        floor += DOWN_FLOOR * above;
        if step > 1 {
            let level = above * 4.0;
            spread += UP_SPREAD * level;
            floor += UP_FLOOR * level;
        }
    }
    (spread, floor)
}

const DOWN_MARGIN: f32 = 1.5;
const UP_MARGIN: f32 = 0.75;
impl PostEffect for Blur {
    fn name(&self) -> &str {
        "blur"
    }

    fn passes(&self, _region: Size, list: &mut PassList) {
        let Some((steps, offset)) = self.chain() else {
            return;
        };
        for step in 1..=steps {
            list.push(
                BackdropPass::new(DOWN_SHADER.clone())
                    .params(&[offset])
                    .scale(scale_at(step))
                    .taps(offset + DOWN_MARGIN)
                    .extent(PassExtent::Plan),
            );
        }
        for step in (1..steps).rev() {
            list.push(
                BackdropPass::new(UP_SHADER.clone())
                    .params(&[offset])
                    .scale(scale_at(step))
                    .taps(0.5 * offset + UP_MARGIN)
                    .extent(PassExtent::Plan),
            );
        }
    }
}

pub(crate) fn shaders() -> [BackdropShader; 2] {
    [DOWN_SHADER.clone(), UP_SHADER.clone()]
}

fn scale_at(step: u32) -> f32 {
    1.0 / (1u32 << step) as f32
}
