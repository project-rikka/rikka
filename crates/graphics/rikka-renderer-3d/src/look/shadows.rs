use crate::look::Tier;

/// Shadow settings of a looked frame.
///
/// `tier` picks shadow maps (`ScreenSpace`), ray-traced visibility (`RayTraced`, falling back
/// per light to the maps for lights the tracer does not cover) or none (`Off`). Cascaded maps
/// cover the leading directional light out to `distance`, filtered with PCSS from the light's
/// [`source_size`](crate::Light::source_size); figures get their own maps (see
/// [`Figure::shadow`](crate::Figure::shadow)); `screen` adds fine contact shadows traced in
/// screen space from the leading light. `samples` is the ray count per pixel and light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadows {
    pub tier: Tier,
    /// How far from the camera the directional cascades reach, in world units.
    pub distance: f32,
    pub samples: u32,
    pub screen: ScreenShadows,
}

impl Default for Shadows {
    fn default() -> Self {
        Self {
            tier: Tier::Auto,
            distance: 120.0,
            samples: 1,
            screen: ScreenShadows::default(),
        }
    }
}

/// Screen-space shadows from the leading directional light (Bend Studio's wavefront march).
///
/// Each pixel looks 32 pixels toward the light along the depth buffer; a sample closer to the
/// camera than the light ray, by less than `thickness`, shadows it. Off at strength 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenShadows {
    pub strength: f32,
    /// Assumed depth of what the depth buffer shows, in world units; thicker surfaces cast
    /// longer screen shadows, thinner ones leak less.
    pub thickness: f32,
}

impl Default for ScreenShadows {
    fn default() -> Self {
        Self {
            strength: 1.0,
            thickness: 0.05,
        }
    }
}
