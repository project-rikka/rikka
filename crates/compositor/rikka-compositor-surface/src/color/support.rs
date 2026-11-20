use rikka_compositor_protocols::wp_color_manager_v1::{
    Feature, Primaries, RenderIntent, TransferFunction,
};

use super::description::Parametric;

#[derive(Debug, Clone, PartialEq)]
pub struct ColorSupport {
    pub intents: Vec<RenderIntent>,
    pub features: Vec<Feature>,
    pub transfer_functions: Vec<TransferFunction>,
    pub primaries: Vec<Primaries>,
    pub preferred: Parametric,
}

impl Default for ColorSupport {
    fn default() -> Self {
        Self {
            intents: Vec::new(),
            features: Vec::new(),
            transfer_functions: Vec::new(),
            primaries: Vec::new(),
            preferred: Parametric::named(Primaries::Srgb, TransferFunction::Gamma22)
                .expect("gamma 2.2 is a known transfer function"),
        }
    }
}

impl ColorSupport {
    pub(crate) fn has(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }
}
