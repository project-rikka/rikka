use std::ops::Range;
use std::sync::{Arc, Weak};

use rikka_spirv::Shader;
use rikka_types::{Rect, Size};

use crate::fx::pipelines;

const CUSTOM_LABEL: &str = "rikka.fx.effect";

#[derive(Clone, Debug)]
#[must_use]
pub struct BackdropShader {
    inner: Arc<ShaderInner>,
}

#[derive(Debug)]
struct ShaderInner {
    shader: Shader,
    label: &'static str,
}

pub(crate) struct WeakShader(Weak<ShaderInner>);

impl WeakShader {
    pub(crate) fn alive(&self) -> bool {
        self.0.strong_count() > 0
    }

    pub(crate) fn upgrade(&self) -> Option<BackdropShader> {
        self.0.upgrade().map(|inner| BackdropShader { inner })
    }
}

impl BackdropShader {
    pub fn new(shader: Shader) -> Self {
        let backdrop = Self::builtin(shader, CUSTOM_LABEL);
        pipelines::precompile_all(&backdrop);
        backdrop
    }

    pub(crate) fn builtin(shader: Shader, label: &'static str) -> Self {
        Self {
            inner: Arc::new(ShaderInner { shader, label }),
        }
    }

    pub(crate) fn spirv(&self) -> &'static [u8] {
        self.inner.shader.spirv
    }

    pub(crate) fn hash(&self) -> u64 {
        self.inner.shader.hash
    }

    pub(crate) fn label(&self) -> &'static str {
        self.inner.label
    }

    pub(crate) fn downgrade(&self) -> WeakShader {
        WeakShader(Arc::downgrade(&self.inner))
    }
}

pub(crate) const PARAMS: usize = 20;

const INPUT_CLAMP: Range<usize> = 4..8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PassExtent {
    Input,
    Plan,
}

#[derive(Clone, Debug)]
#[must_use]
pub struct BackdropPass {
    pub(crate) shader: BackdropShader,
    pub(crate) params: [f32; PARAMS],
    pub(crate) scale: f32,
    pub(crate) taps: f32,
    pub(crate) core_taps: f32,
    pub(crate) stale_reach: f32,
    pub(crate) core_stale_reach: f32,
    pub(crate) corners_at: Option<usize>,
    pub(crate) extent: PassExtent,
}

impl BackdropPass {
    pub fn new(shader: BackdropShader) -> Self {
        Self {
            shader,
            params: [0.0; PARAMS],
            scale: 1.0,
            taps: 0.0,
            core_taps: 0.0,
            stale_reach: 0.0,
            core_stale_reach: 0.0,
            corners_at: None,
            extent: PassExtent::Input,
        }
    }

    pub fn params(mut self, values: &[f32]) -> Self {
        assert!(
            values.len() <= PARAMS,
            "effect pass parameters exceed {PARAMS} floats"
        );
        self.params[..values.len()].copy_from_slice(values);
        self
    }

    pub fn scale(mut self, scale: f32) -> Self {
        assert!(scale > 0.0 && scale <= 1.0, "scale must be in (0, 1]");
        self.scale = scale;
        self
    }

    pub fn taps(mut self, texels: f32) -> Self {
        self.taps = texels.max(0.0);
        self.core_taps = self.taps;
        self
    }

    pub(crate) fn core_taps(mut self, texels: f32) -> Self {
        self.core_taps = texels.clamp(0.0, self.taps);
        self
    }

    pub(crate) fn corners_at(mut self, index: usize) -> Self {
        self.corners_at = Some(index);
        self
    }

    pub(crate) fn extent(mut self, extent: PassExtent) -> Self {
        self.extent = extent;
        self
    }

    pub(crate) fn target(&self, first: bool, input: Rect, plan: Rect) -> Rect {
        match (first, self.extent) {
            (true, PassExtent::Input) => input,
            _ => plan,
        }
    }

    pub(crate) fn params_clamped_to(&self, input_bounds: [f32; 4]) -> [f32; PARAMS] {
        let mut params = self.params;
        if self.extent == PassExtent::Plan {
            params[INPUT_CLAMP].copy_from_slice(&input_bounds);
        }
        params
    }

    pub(crate) fn signed_params(&self) -> impl Iterator<Item = f32> + '_ {
        let corners = self.corners_at.map_or(0..0, |at| at..at + 4);
        self.params
            .iter()
            .enumerate()
            .filter(move |(index, _)| !corners.contains(index))
            .map(|(_, value)| *value)
    }
}
#[derive(Debug, Default)]
pub struct PassList {
    pub(crate) passes: Vec<BackdropPass>,
}

impl PassList {
    pub fn push(&mut self, pass: BackdropPass) {
        self.passes.push(pass);
    }
}

fn read_at(passes: &[BackdropPass], index: usize) -> f32 {
    match index {
        0 => 1.0,
        _ => passes[index - 1].scale,
    }
}

pub(crate) fn spread_of(passes: &[BackdropPass]) -> f32 {
    reached(passes, |pass| pass.taps)
}

pub(crate) fn core_spread_of(passes: &[BackdropPass]) -> f32 {
    reached(passes, |pass| pass.core_taps)
}

pub(crate) fn core_reach(passes: &[BackdropPass]) -> f32 {
    let first = &passes[0];
    first.core_taps + first.core_stale_reach + rounding_of(passes)
}

fn rounding_of(passes: &[BackdropPass]) -> f32 {
    passes.iter().map(|pass| pass.scale.recip()).sum()
}

pub(crate) fn first_write_rounding(passes: &[BackdropPass]) -> f32 {
    passes.first().map_or(0.0, |pass| pass.scale.recip())
}

fn reached(passes: &[BackdropPass], taps: impl Fn(&BackdropPass) -> f32) -> f32 {
    (0..passes.len())
        .map(|index| taps(&passes[index]) / read_at(passes, index))
        .sum()
}

pub(crate) fn accumulate_reach(passes: &mut [BackdropPass]) {
    let (mut stale_reach, mut core_stale_reach) = (0.0, 0.0);
    for index in (0..passes.len()).rev() {
        let at = read_at(passes, index);
        let pass = &mut passes[index];
        pass.stale_reach = stale_reach;
        pass.core_stale_reach = core_stale_reach;
        stale_reach += pass.taps / at;
        core_stale_reach += pass.core_taps / at;
    }
}

pub trait PostEffect: Send + Sync + 'static {
    fn name(&self) -> &str {
        "effect"
    }

    fn passes(&self, region: Size, list: &mut PassList);

    fn spread(&self, region: Size) -> f32 {
        let mut list = PassList::default();
        self.passes(region, &mut list);
        spread_of(&list.passes)
    }
}

impl std::fmt::Debug for dyn PostEffect {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PostEffect({})", self.name())
    }
}
