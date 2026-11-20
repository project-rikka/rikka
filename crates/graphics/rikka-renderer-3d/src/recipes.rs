use ash::vk;

use crate::material::features::{ALL_FEATURES, Routing};
use crate::material::{MaterialPass, MaterialState, Program};
use crate::pipeline::{PassFormats, PipelineKey, Raster, Recipe};
use crate::shader::{MaterialShader, Tracing};
use crate::shadow::store::TINT_FORMAT;
use crate::state::Builtin;
use crate::viewport::{COLOR_FORMAT, DEPTH_FORMAT, IDENTITY_FORMAT, OUTLINE_FORMAT};
use rikka_gpu::core::pipeline::Blend;
use rikka_gpu::texture::OUTPUT_FORMAT;
use rikka_shader_programs::Variant;

const SHADOW_BIAS: i32 = -2;
const SHADOW_SLOPE: f32 = -1.5;

pub(crate) const EMISSION_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
pub(crate) const OUTLINE_ID_FORMAT: vk::Format = vk::Format::R32_UINT;
pub(crate) const OUTLINE_COLOR_FORMAT: vk::Format = vk::Format::R8G8B8A8_UNORM;
pub(crate) const OUTLINE_LINE_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ViewFormats {
    pub samples: vk::SampleCountFlags,
    pub identity: bool,
    pub emission: bool,
    pub tracing: Tracing,
}

impl ViewFormats {
    fn prepass(self) -> PassFormats {
        match self.identity {
            true => PassFormats::color(IDENTITY_FORMAT, DEPTH_FORMAT, self.samples),
            false => PassFormats::depth_only(DEPTH_FORMAT, self.samples),
        }
    }

    pub(crate) fn shade(self) -> PassFormats {
        self.shading(false)
    }

    fn shading(self, emits: bool) -> PassFormats {
        let formats = PassFormats::color(COLOR_FORMAT, DEPTH_FORMAT, self.samples);
        match self.emission {
            true => formats.with_second(EMISSION_FORMAT, emits),
            false => formats,
        }
    }
}

pub(crate) fn recipe(
    vertex: &MaterialShader,
    fragment: Option<&MaterialShader>,
    raster: Raster,
    formats: PassFormats,
    masked: bool,
) -> Recipe {
    Recipe {
        key: PipelineKey {
            vertex: vertex.hash(),
            fragment: fragment.map(MaterialShader::hash),
            raster,
            formats,
            masked,
            features: ALL_FEATURES,
            clipping: false,
        },
        vertex: vertex.clone(),
        fragment: fragment.cloned(),
        clips: false,
    }
}

pub(crate) fn prepass(
    builtin: &Builtin,
    program: &Program,
    state: MaterialState,
    masked: bool,
    view: ViewFormats,
) -> Recipe {
    let fragment = match (view.identity, masked) {
        (true, true) => Some(program.shader(Variant::Depth)),
        (true, false) => Some(&builtin.depth_frag),
        (false, true) => Some(program.shader(Variant::Cutout)),
        (false, false) => None,
    };
    recipe(
        depth_vertex(builtin, Some(program)),
        fragment,
        Raster::depth_of(state),
        view.prepass(),
        masked,
    )
    .clippable()
}

fn depth_vertex<'a>(builtin: &'a Builtin, program: Option<&'a Program>) -> &'a MaterialShader {
    match program.filter(|program| program.deforms) {
        Some(program) => program.shader(Variant::DepthVertex),
        None => &builtin.depth_vert,
    }
}

pub(crate) fn shadow(builtin: &Builtin, program: Option<&Program>, masked: bool) -> Recipe {
    recipe(
        depth_vertex(builtin, program),
        program
            .filter(|_| masked)
            .map(|program| program.shader(Variant::Cutout)),
        shadow_raster(),
        PassFormats::shadow(DEPTH_FORMAT, vk::SampleCountFlags::TYPE_1),
        masked,
    )
}

pub(crate) fn shadow_tint(builtin: &Builtin, program: &Program) -> Recipe {
    recipe(
        depth_vertex(builtin, Some(program)),
        Some(program.shader(Variant::Tint)),
        Raster::blended(Blend::Multiply),
        PassFormats::color(
            TINT_FORMAT,
            vk::Format::UNDEFINED,
            vk::SampleCountFlags::TYPE_1,
        ),
        true,
    )
}

pub(crate) fn shadow_raster() -> Raster {
    Raster::biased(SHADOW_BIAS, SHADOW_SLOPE)
}

pub(crate) fn character_depth(
    builtin: &Builtin,
    program: &Program,
    state: MaterialState,
) -> Recipe {
    recipe(
        depth_vertex(builtin, Some(program)),
        Some(program.shader(Variant::Cutout)),
        Raster::depth_of(state),
        PassFormats::depth_only(DEPTH_FORMAT, vk::SampleCountFlags::TYPE_1),
        true,
    )
}

pub(crate) fn see_through(
    builtin: &Builtin,
    program: &Program,
    state: MaterialState,
    features: u32,
    view: ViewFormats,
) -> Recipe {
    shade(
        builtin,
        program,
        false,
        MaterialState {
            blend: Blend::Alpha,
            depth_write: false,
            ..state
        },
        features,
        view,
    )
}

pub(crate) fn shade(
    builtin: &Builtin,
    program: &Program,
    hull: bool,
    state: MaterialState,
    features: u32,
    view: ViewFormats,
) -> Recipe {
    let emits = view.emission && !hull;
    let (vertex, fragment) = match hull {
        true => (
            program.shader(Variant::HullVertex),
            program.shader(match view.tracing {
                Tracing::RayQueries => Variant::HullTraced,
                Tracing::Raster => Variant::Hull,
            }),
        ),
        false => (
            match program.deforms {
                true => program.shader(Variant::ForwardVertex),
                false => &builtin.mesh_vert,
            },
            program.shader(match (emits, view.tracing) {
                (true, Tracing::RayQueries) => Variant::ForwardEmissiveTraced,
                (true, Tracing::Raster) => Variant::ForwardEmissive,
                (false, Tracing::RayQueries) => Variant::ForwardTraced,
                (false, Tracing::Raster) => Variant::Forward,
            }),
        ),
    };
    featured(
        recipe(
            vertex,
            Some(fragment),
            Raster::from_state(state),
            view.shading(emits),
            false,
        ),
        features,
    )
    .clippable()
}

pub(crate) fn scatter(
    builtin: &Builtin,
    program: &Program,
    state: MaterialState,
    features: u32,
) -> Recipe {
    let vertex = match program.deforms {
        true => program.shader(Variant::ForwardVertex),
        false => &builtin.mesh_vert,
    };
    featured(
        recipe(
            vertex,
            Some(program.shader(Variant::Scatter)),
            Raster {
                blend: Blend::Opaque,
                alpha_to_coverage: false,
                ..Raster::from_state(state)
            },
            PassFormats::color(COLOR_FORMAT, DEPTH_FORMAT, vk::SampleCountFlags::TYPE_1)
                .with_second(COLOR_FORMAT, true),
            false,
        ),
        features,
    )
}

fn featured(mut recipe: Recipe, features: u32) -> Recipe {
    recipe.key.features = features;
    recipe
}

pub(crate) fn surfaces(
    vertex: &MaterialShader,
    program: &Program,
    state: MaterialState,
    formats: PassFormats,
    layers: bool,
    masked: bool,
) -> Recipe {
    let vertex = match program.deforms {
        true => program.shader(Variant::SurfacesVertex),
        false => vertex,
    };
    let fragment = match layers {
        true => program.shader(Variant::SurfacesLayers),
        false => program.shader(Variant::Surfaces),
    };
    recipe(
        vertex,
        Some(fragment),
        Raster::depth_of(state),
        formats,
        masked,
    )
    .clippable()
}

pub(crate) fn sky(builtin: &Builtin, view: ViewFormats) -> Recipe {
    fullscreen(builtin, &builtin.sky_frag, Raster::FILL, view.shade())
}

pub(crate) fn clip_bounds(builtin: &Builtin, view: ViewFormats) -> Recipe {
    fullscreen(
        builtin,
        &builtin.clip_bounds_frag,
        Raster::FILL,
        view.shade(),
    )
}

pub(crate) fn grid(builtin: &Builtin, view: ViewFormats) -> Recipe {
    fullscreen(
        builtin,
        &builtin.grid_frag,
        Raster::blended(Blend::Premultiplied),
        view.shade(),
    )
}

pub(crate) fn tonemap(builtin: &Builtin) -> Recipe {
    fullscreen(
        builtin,
        &builtin.tonemap_frag,
        Raster::FILL,
        PassFormats::color(
            OUTPUT_FORMAT,
            vk::Format::UNDEFINED,
            vk::SampleCountFlags::TYPE_1,
        ),
    )
}

pub(crate) fn restore(builtin: &Builtin, view: ViewFormats) -> Recipe {
    fullscreen(builtin, &builtin.copy_frag, Raster::FILL, view.shade())
}

pub(crate) fn outline(builtin: &Builtin) -> Recipe {
    fullscreen(
        builtin,
        &builtin.outline_frag,
        Raster::FILL,
        PassFormats::color(
            OUTLINE_FORMAT,
            vk::Format::UNDEFINED,
            vk::SampleCountFlags::TYPE_1,
        ),
    )
}

pub(crate) fn fullscreen(
    builtin: &Builtin,
    fragment: &MaterialShader,
    raster: Raster,
    formats: PassFormats,
) -> Recipe {
    recipe(
        &builtin.fullscreen_vert,
        Some(fragment),
        raster,
        formats,
        false,
    )
}

pub(crate) fn frame_wide(builtin: &Builtin) -> [Recipe; 3] {
    [
        shadow(builtin, None, false),
        tonemap(builtin),
        outline(builtin),
    ]
}

pub(crate) fn view_wide(builtin: &Builtin, view: ViewFormats) -> [Recipe; 3] {
    [
        opaque_prepass(builtin, MaterialState::opaque(), view),
        sky(builtin, view),
        grid(builtin, view),
    ]
}

pub(crate) fn opaque_prepass(builtin: &Builtin, state: MaterialState, view: ViewFormats) -> Recipe {
    recipe(
        &builtin.depth_vert,
        view.identity.then_some(&builtin.depth_frag),
        Raster::depth_of(state),
        view.prepass(),
        false,
    )
    .clippable()
}

pub(crate) fn material(
    builtin: &Builtin,
    program: &Program,
    (state, hull, routing): (MaterialState, bool, Routing),
    view: ViewFormats,
) -> Vec<Recipe> {
    let mut recipes = vec![shade(builtin, program, hull, state, routing.features, view)];
    if state.pass.in_prepass() && !hull {
        let masked = state.pass == MaterialPass::Masked;
        recipes.push(prepass(builtin, program, state, masked, view));
        if (masked || program.deforms) && state.casts_shadow {
            recipes.push(shadow(builtin, Some(program), masked));
        }
        if program.scatters && routing.subsurface {
            recipes.push(scatter(builtin, program, state, routing.features));
        }
    }
    recipes
}

pub(crate) fn filter(
    builtin: &Builtin,
    fragment: &MaterialShader,
    blend: Blend,
    format: vk::Format,
) -> Recipe {
    fullscreen(
        builtin,
        fragment,
        Raster::blended(blend),
        PassFormats::color(format, vk::Format::UNDEFINED, vk::SampleCountFlags::TYPE_1),
    )
}

pub(crate) fn occluder(builtin: &Builtin, program: &Program, state: MaterialState) -> Recipe {
    let masked = state.pass == MaterialPass::Masked;
    recipe(
        depth_vertex(builtin, Some(program)),
        masked.then(|| program.shader(Variant::Cutout)),
        Raster::depth_of(state),
        PassFormats::depth_only(DEPTH_FORMAT, vk::SampleCountFlags::TYPE_1),
        masked,
    )
    .clippable()
}

pub(crate) fn overlay(shaders: &[MaterialShader; 2]) -> Recipe {
    recipe(
        &shaders[0],
        Some(&shaders[1]),
        Raster::blended(Blend::Premultiplied),
        PassFormats::color(OUTPUT_FORMAT, DEPTH_FORMAT, vk::SampleCountFlags::TYPE_1),
        false,
    )
    .clippable()
}

pub(crate) fn overlays(builtin: &Builtin) -> [Recipe; 3] {
    [
        overlay(&builtin.overlay_lines),
        overlay(&builtin.overlay_points),
        overlay(&builtin.overlay_triangles),
    ]
}

pub(crate) fn outline_prepass(
    builtin: &Builtin,
    program: &Program,
    state: MaterialState,
) -> Recipe {
    recipe(
        depth_vertex(builtin, Some(program)),
        Some(&builtin.outline_id_frag),
        Raster::depth_of(state),
        PassFormats::color(
            OUTLINE_ID_FORMAT,
            DEPTH_FORMAT,
            vk::SampleCountFlags::TYPE_1,
        ),
        false,
    )
    .clippable()
}

pub(crate) fn outline_edges(builtin: &Builtin) -> Recipe {
    recipe(
        &builtin.outline_edges[0],
        Some(&builtin.outline_edges[1]),
        Raster::FILL,
        PassFormats::color(
            OUTLINE_ID_FORMAT,
            DEPTH_FORMAT,
            vk::SampleCountFlags::TYPE_1,
        ),
        false,
    )
    .clippable()
}

pub(crate) fn outline_detect(builtin: &Builtin) -> Recipe {
    fullscreen(
        builtin,
        &builtin.outline_detect_frag,
        Raster::FILL,
        PassFormats::pair(
            OUTLINE_COLOR_FORMAT,
            OUTLINE_LINE_FORMAT,
            vk::SampleCountFlags::TYPE_1,
        ),
    )
}

pub(crate) fn outline_resolve(builtin: &Builtin) -> Recipe {
    fullscreen(
        builtin,
        &builtin.outline_resolve_frag,
        Raster::blended(Blend::Premultiplied),
        PassFormats::color(
            OUTPUT_FORMAT,
            vk::Format::UNDEFINED,
            vk::SampleCountFlags::TYPE_1,
        ),
    )
}
