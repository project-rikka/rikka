use crate::rect::Rect;
use crate::size::Size;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RefractionExtent {
    pub sample_margin: f32,
    pub paint_margin: f32,
}

impl RefractionExtent {
    pub const NONE: RefractionExtent = RefractionExtent {
        sample_margin: 0.0,
        paint_margin: 0.0,
    };
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RefractionProfile {
    pub bevel_width: f32,
    pub bevel_displacement: f32,
    pub bulge_displacement: f32,
}

impl RefractionProfile {
    pub const NONE: RefractionProfile = RefractionProfile {
        bevel_width: 0.0,
        bevel_displacement: 0.0,
        bulge_displacement: 0.0,
    };

    #[must_use]
    pub fn max_displacement(self) -> f32 {
        self.bevel_displacement.max(self.bulge_displacement)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[must_use]
pub struct Lens {
    bevel: f32,
    depth: f32,
    bulge: f32,
    frost: f32,
    ior: f32,
    dispersion: f32,
    adapt: f32,
}

impl Lens {
    pub const CROWN_GLASS_IOR: f32 = 1.52;

    const DEFAULT_BEVEL: f32 = 64.0;

    const DEFAULT_DEPTH: f32 = 210.0;

    const DEFAULT_BULGE: f32 = 0.05;

    const DEFAULT_FROST: f32 = 0.4;

    const DEFAULT_DISPERSION: f32 = 0.40;

    const DEFAULT_ADAPT: f32 = 0.35;
}

impl Lens {
    pub fn new() -> Lens {
        Lens {
            bevel: Lens::DEFAULT_BEVEL,
            depth: Lens::DEFAULT_DEPTH,
            bulge: Lens::DEFAULT_BULGE,
            frost: Lens::DEFAULT_FROST,
            ior: Lens::CROWN_GLASS_IOR,
            dispersion: Lens::DEFAULT_DISPERSION,
            adapt: Lens::DEFAULT_ADAPT,
        }
    }

    pub fn with_bevel(mut self, pixels: f32) -> Lens {
        self.bevel = pixels.max(0.0);
        self
    }

    pub fn with_depth(mut self, pixels: f32) -> Lens {
        self.depth = pixels.max(0.0);
        self
    }

    pub fn with_bulge(mut self, bulge: f32) -> Lens {
        self.bulge = clamp_unit(bulge);
        self
    }

    pub fn with_frost(mut self, frost: f32) -> Lens {
        self.frost = clamp_unit(frost);
        self
    }

    pub fn with_ior(mut self, ior: f32) -> Lens {
        self.ior = ior.max(1.0);
        self
    }

    pub fn with_dispersion(mut self, dispersion: f32) -> Lens {
        self.dispersion = clamp_unit(dispersion);
        self
    }

    pub fn with_adapt(mut self, adapt: f32) -> Lens {
        self.adapt = clamp_unit(adapt);
        self
    }

    pub fn scaled(mut self, factor: f32) -> Lens {
        let factor = factor.max(0.0);
        self.bevel *= factor;
        self.depth *= factor;
        self
    }

    #[must_use]
    pub fn refracts(self) -> bool {
        self.ior > 1.0 && (self.bulge > 0.0 || (self.bevel > 0.0 && self.depth > 0.0))
    }

    #[must_use]
    pub fn bevel(self) -> f32 {
        self.bevel
    }

    #[must_use]
    pub fn depth(self) -> f32 {
        self.depth
    }

    #[must_use]
    pub fn bulge(self) -> f32 {
        self.bulge
    }

    #[must_use]
    pub fn frost(self) -> f32 {
        self.frost
    }

    #[must_use]
    pub fn ior(self) -> f32 {
        self.ior
    }

    #[must_use]
    pub fn refraction_strength(self) -> f32 {
        1.0 - 1.0 / self.ior
    }

    #[must_use]
    pub fn dispersion(self) -> f32 {
        self.dispersion
    }

    #[must_use]
    pub fn adapt(self) -> f32 {
        self.adapt
    }

    #[must_use]
    pub fn extent_for_size(self, size: Size) -> RefractionExtent {
        if !self.refracts() {
            return RefractionExtent::NONE;
        }
        let profile = self.profile_for_size(size);
        let Size {
            width: half_width,
            height: half_height,
        } = half_size(size);
        let bulge_inset = (1.0 - self.max_dispersion_scale() * self.bulge).abs() - 1.0;
        RefractionExtent {
            sample_margin: (profile.bevel_displacement + bulge_inset * half_width)
                .max(profile.bevel_displacement + bulge_inset * half_height)
                .max(0.0),
            paint_margin: profile.max_displacement(),
        }
    }

    #[must_use]
    pub fn extent_within(self, glass: Rect, pane: Rect) -> RefractionExtent {
        if !self.refracts() {
            return RefractionExtent::NONE;
        }
        let profile = self.profile_within(glass, pane);
        let centre = glass.center();
        let bulge = self.max_dispersion_scale() * self.bulge;
        let overshoot = |near: f32, far: f32, at: f32| {
            let from_near = near - (near - at) * bulge;
            let from_far = far - (far - at) * bulge;
            (near - from_near.min(from_far))
                .max(from_near.max(from_far) - far)
                .max(0.0)
        };
        let swell_outside = overshoot(pane.min_x(), pane.max_x(), centre.x).max(overshoot(
            pane.min_y(),
            pane.max_y(),
            centre.y,
        ));
        RefractionExtent {
            sample_margin: profile.bevel_displacement + swell_outside,
            paint_margin: profile.bevel_displacement + profile.bulge_displacement,
        }
    }

    #[must_use]
    pub fn profile_for_size(self, size: Size) -> RefractionProfile {
        let Size { width, height } = half_size(size);
        self.profile_spanning(size, width.max(height))
    }

    #[must_use]
    pub fn profile_within(self, glass: Rect, pane: Rect) -> RefractionProfile {
        let centre = glass.center();
        let farthest = |near: f32, far: f32, at: f32| (near - at).abs().max((far - at).abs());
        let extent = farthest(pane.min_x(), pane.max_x(), centre.x).max(farthest(
            pane.min_y(),
            pane.max_y(),
            centre.y,
        ));
        self.profile_spanning(glass.size, extent)
    }

    fn profile_spanning(self, glass: Size, extent: f32) -> RefractionProfile {
        if !self.refracts() {
            return RefractionProfile::NONE;
        }
        let bevel_width = self.bevel.min(half_size(glass).min_side());
        let bevel_offset = match self.bevel > 0.0 {
            true => self.depth * bevel_width / self.bevel,
            false => 0.0,
        };
        RefractionProfile {
            bevel_width: match bevel_offset > 0.0 {
                true => bevel_width,
                false => 0.0,
            },
            bevel_displacement: self.max_dispersion_scale() * bevel_offset,
            bulge_displacement: self.max_dispersion_scale() * self.bulge * extent.max(0.0),
        }
    }

    fn max_dispersion_scale(self) -> f32 {
        1.0 + self.dispersion
    }
}

impl Default for Lens {
    fn default() -> Lens {
        Lens::new()
    }
}

fn half_size(size: Size) -> Size {
    Size::new((size.width * 0.5).max(0.0), (size.height * 0.5).max(0.0))
}
#[expect(
    clippy::manual_clamp,
    reason = "clamp passes a NaN through and this must not"
)]
fn clamp_unit(value: f32) -> f32 {
    value.max(0.0).min(1.0)
}
