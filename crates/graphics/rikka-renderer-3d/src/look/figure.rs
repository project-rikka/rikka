use glam::Vec3;

use crate::light::{LightRig, ShadowQuality};
use crate::material::Overrides;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Head {
    pub center: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
    pub radius: f32,
}

/// One character of a frame: its head for facial shading, material overrides, its own light
/// rig and its own shadow map.
///
/// `shadow` sizes a map fitted tightly around the figure's drawn bounds from the leading
/// directional light (or from its rig's turned key). The figure leaves the world cascades for
/// that light and casts through this map instead, so its self-shadowing and the shadow it
/// lays on the ground stay sharp at any camera distance. `ShadowQuality::None` keeps it in the
/// cascades. Draws with unbounded deformation (drapes, outside deformers) do not widen the fit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Figure {
    pub head: Option<Head>,
    pub overrides: Overrides,
    pub rig: LightRig,
    pub shadow: ShadowQuality,
}

impl Figure {
    pub fn new(overrides: Overrides) -> Self {
        Self {
            overrides,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FigureId(pub(crate) u32);
