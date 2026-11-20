use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use rikka_gpu::core::pipeline::Blend;

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum MaterialPass {
    #[default]
    Opaque,
    Masked,
    Transparent,
    Overlay,
}

impl MaterialPass {
    pub(crate) fn in_prepass(self) -> bool {
        matches!(self, Self::Opaque | Self::Masked)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cull {
    #[default]
    Back,
    Front,
    None,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Polygon {
    #[default]
    Fill,
    Line,
    Point,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MaterialState {
    pub pass: MaterialPass,
    pub cull: Cull,
    pub blend: Blend,
    pub polygon: Polygon,
    pub depth_test: bool,
    pub depth_write: bool,
    pub depth_bias: i32,
    pub depth_bias_slope: f32,
    pub sample_shading: bool,
    pub alpha_to_coverage: bool,
    pub double_sided: bool,
    pub casts_shadow: bool,
    pub receives_shadow: bool,
}

impl Default for MaterialState {
    fn default() -> Self {
        Self {
            pass: MaterialPass::Opaque,
            cull: Cull::Back,
            blend: Blend::Opaque,
            polygon: Polygon::Fill,
            depth_test: true,
            depth_write: true,
            depth_bias: 0,
            depth_bias_slope: 0.0,
            sample_shading: false,
            alpha_to_coverage: false,
            double_sided: false,
            casts_shadow: true,
            receives_shadow: true,
        }
    }
}

impl Eq for MaterialState {}

impl std::hash::Hash for MaterialState {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let Self {
            pass,
            cull,
            blend,
            polygon,
            depth_test,
            depth_write,
            depth_bias,
            depth_bias_slope,
            sample_shading,
            alpha_to_coverage,
            double_sided,
            casts_shadow,
            receives_shadow,
        } = self;
        (pass, cull, blend, polygon).hash(state);
        (depth_test, depth_write, depth_bias).hash(state);
        depth_bias_slope.to_bits().hash(state);
        (sample_shading, alpha_to_coverage, double_sided).hash(state);
        (casts_shadow, receives_shadow).hash(state);
    }
}

impl MaterialState {
    pub fn opaque() -> Self {
        Self::default()
    }

    pub fn masked() -> Self {
        Self {
            pass: MaterialPass::Masked,
            ..Self::default()
        }
    }

    pub fn transparent() -> Self {
        Self {
            pass: MaterialPass::Transparent,
            blend: Blend::Alpha,
            depth_write: false,
            casts_shadow: false,
            ..Self::default()
        }
    }

    pub fn coverage() -> Self {
        Self {
            pass: MaterialPass::Transparent,
            blend: Blend::Opaque,
            alpha_to_coverage: true,
            ..Self::default()
        }
    }

    pub fn overlay() -> Self {
        Self {
            pass: MaterialPass::Overlay,
            blend: Blend::Premultiplied,
            depth_write: false,
            casts_shadow: false,
            receives_shadow: false,
            ..Self::default()
        }
    }

    pub fn wireframe() -> Self {
        Self {
            polygon: Polygon::Line,
            depth_bias: -64,
            depth_bias_slope: -1.0,
            ..Self::overlay()
        }
    }

    pub fn pass(mut self, pass: MaterialPass) -> Self {
        self.pass = pass;
        self
    }

    pub fn cull(mut self, cull: Cull) -> Self {
        self.cull = cull;
        self
    }

    pub fn blend(mut self, blend: Blend) -> Self {
        self.blend = blend;
        self
    }

    pub fn polygon(mut self, polygon: Polygon) -> Self {
        self.polygon = polygon;
        self
    }

    pub fn depth(mut self, test: bool, write: bool) -> Self {
        self.depth_test = test;
        self.depth_write = write;
        self
    }

    pub fn depth_bias(mut self, constant: i32, slope: f32) -> Self {
        self.depth_bias = constant;
        self.depth_bias_slope = slope;
        self
    }

    pub fn double_sided(mut self) -> Self {
        self.double_sided = true;
        self.cull = Cull::None;
        self
    }

    pub fn shadows(mut self, casts: bool, receives: bool) -> Self {
        self.casts_shadow = casts;
        self.receives_shadow = receives;
        self
    }

    pub fn sample_shading(mut self) -> Self {
        self.sample_shading = true;
        self
    }

    pub fn alpha_to_coverage(mut self) -> Self {
        self.alpha_to_coverage = true;
        self
    }
}

impl MaterialState {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.polygon != Polygon::Fill && self.pass.in_prepass() {
            return Err(Error::Invalid(
                "line and point materials cannot use the depth prepass; draw them in the \
                 transparent or overlay pass"
                    .into(),
            ));
        }
        Ok(())
    }
}
