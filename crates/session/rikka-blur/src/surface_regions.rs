use rikka_types::RoundedRect;

use crate::blurred_surface::BlurredSurface;
use crate::opaque_region::OpaqueRegion;

#[derive(Debug, Default)]
pub struct SurfaceRegions {
    blurred: Option<BlurredSurface>,
    opaque_region: Option<OpaqueRegion>,
    blurs: bool,
    blur: Vec<RoundedRect>,
    opaque: Vec<RoundedRect>,
}

impl SurfaceRegions {
    #[must_use]
    pub fn new(blurred: Option<BlurredSurface>, blurs: bool) -> Self {
        Self {
            blurred,
            opaque_region: None,
            blurs,
            blur: Vec::new(),
            opaque: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_opaque_region(mut self, opaque_region: Option<OpaqueRegion>) -> Self {
        self.opaque_region = opaque_region;
        self
    }

    pub fn set_regions(&mut self, blur: &[RoundedRect], opaque: &[RoundedRect]) {
        self.blur.clear();
        if self.blurs {
            self.blur.extend_from_slice(blur);
        }
        self.opaque.clear();
        self.opaque.extend_from_slice(opaque);
    }

    pub fn apply(&self) {
        if let Some(blurred) = &self.blurred {
            blurred.set_region(&self.blur);
        }
        if let Some(opaque_region) = &self.opaque_region {
            opaque_region.set_region(&self.opaque, &self.blur);
        }
    }

    pub fn set_blur(&mut self, blurs: bool, create: impl FnOnce() -> Option<BlurredSurface>) {
        self.blurs = blurs;
        if blurs && self.blurred.is_none() {
            self.blurred = create();
        }
    }
}
