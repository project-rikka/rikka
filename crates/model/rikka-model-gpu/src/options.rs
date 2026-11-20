use rikka_gpu::Filtering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use]
pub struct UploadOptions {
    pub anisotropic: bool,
    pub max_texture_size: Option<u32>,
}

impl Default for UploadOptions {
    fn default() -> Self {
        Self {
            anisotropic: true,
            max_texture_size: None,
        }
    }
}

impl UploadOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn anisotropic(mut self, anisotropic: bool) -> Self {
        self.anisotropic = anisotropic;
        self
    }

    pub fn max_texture_size(mut self, size: Option<u32>) -> Self {
        self.max_texture_size = size;
        self
    }

    pub(crate) fn filtering(&self, sampler: rikka_model_core::Sampler) -> Filtering {
        use rikka_model_core::{Filter, Wrap};

        match (sampler.filter, sampler.wrap, self.anisotropic) {
            (Filter::Linear, Wrap::Repeat, true) => Filtering::AnisotropicRepeat,
            (Filter::Linear, Wrap::Repeat, false) => Filtering::LinearRepeat,
            (Filter::Linear, Wrap::Clamp, _) => Filtering::LinearClamp,
            (Filter::Nearest, Wrap::Repeat, _) => Filtering::NearestRepeat,
            (Filter::Nearest, Wrap::Clamp, _) => Filtering::NearestClamp,
        }
    }
}
