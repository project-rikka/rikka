use std::sync::LazyLock;

use rikka_types::{Lens, RefractionProfile, RoundedRect};

use crate::fx::effect::{BackdropPass, BackdropShader};
use crate::fx::joined::Joined;

static REFRACT_SHADER: LazyLock<BackdropShader> =
    LazyLock::new(|| BackdropShader::builtin(shaders::REFRACT_FRAG, "rikka.fx.refract"));

const CORNERS_AT: usize = 4;

pub(crate) fn bend(
    lens: Lens,
    shape: RoundedRect,
    joined: Option<Joined>,
    profile: RefractionProfile,
    passes: &mut Vec<BackdropPass>,
) {
    let taps = match joined {
        Some(_) => profile.bevel_displacement + profile.bulge_displacement,
        None => profile.max_displacement(),
    };
    let pass = BackdropPass::new(REFRACT_SHADER.clone())
        .params(&optics(lens, shape, joined))
        .taps(taps)
        .core_taps(profile.bulge_displacement);
    let pass = match joined {
        Some(_) => pass,
        None => pass.corners_at(CORNERS_AT),
    };
    passes.insert(0, pass);
}

fn optics(lens: Lens, shape: RoundedRect, joined: Option<Joined>) -> [f32; 19] {
    let glass = joined.map_or(shape, |joined| joined.canonical_host());
    let limb = joined.map_or([0.0; 6], |joined| joined.canonical_limb());
    let rect = glass.rect;
    let radii = glass.radii.clamp_for_size(rect.size).to_array();
    [
        rect.x(),
        rect.y(),
        rect.width(),
        rect.height(),
        radii[0],
        radii[1],
        radii[2],
        radii[3],
        lens.bevel(),
        lens.depth(),
        lens.refraction_strength(),
        lens.dispersion(),
        lens.bulge(),
        limb[0],
        limb[1],
        limb[2],
        limb[3],
        limb[4],
        limb[5],
    ]
}

pub(crate) fn shader() -> BackdropShader {
    REFRACT_SHADER.clone()
}
