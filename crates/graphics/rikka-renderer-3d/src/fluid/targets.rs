use ash::vk;

use crate::graph::{FrameGraph, ImageId, ImageSpec, Size};
use crate::record::stage::FluidImages;
use crate::viewport::DEPTH_FORMAT;

pub(crate) const FRONT_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
pub(crate) const THICKNESS_FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;
const SMOOTHED_FORMAT: vk::Format = vk::Format::R32_SFLOAT;
const COVERAGE_FORMAT: vk::Format = vk::Format::R32_SFLOAT;

pub(crate) const THICKNESS_DIVISOR: u32 = 2;

fn image(graph: &mut FrameGraph<'_>, spec: ImageSpec, published: bool) -> ImageId {
    let image = graph.create(spec);
    if published {
        graph.publish(image);
    }
    image
}

pub(crate) fn declare(graph: &mut FrameGraph<'_>) -> FluidImages {
    let half = Size::Divided(THICKNESS_DIVISOR);
    FluidImages {
        depth: image(
            graph,
            ImageSpec::new("rikka.fluid.depth", DEPTH_FORMAT, Size::Viewport).nearest(),
            true,
        ),
        front: image(
            graph,
            ImageSpec::new("rikka.fluid.front", FRONT_FORMAT, Size::Viewport).nearest(),
            true,
        ),
        smoothed: [
            image(
                graph,
                ImageSpec::new("rikka.fluid.smoothed", SMOOTHED_FORMAT, Size::Viewport).nearest(),
                true,
            ),
            image(
                graph,
                ImageSpec::new("rikka.fluid.smoothed", SMOOTHED_FORMAT, Size::Viewport).nearest(),
                false,
            ),
        ],
        splatted: [
            image(
                graph,
                ImageSpec::new("rikka.fluid.splatted_thickness", THICKNESS_FORMAT, half),
                false,
            ),
            image(
                graph,
                ImageSpec::new("rikka.fluid.splatted_scattering", THICKNESS_FORMAT, half),
                false,
            ),
        ],
        thickness: image(
            graph,
            ImageSpec::new("rikka.fluid.thickness", THICKNESS_FORMAT, half),
            true,
        ),
        scattering: image(
            graph,
            ImageSpec::new("rikka.fluid.scattering", THICKNESS_FORMAT, half),
            true,
        ),
        spread: [
            image(
                graph,
                ImageSpec::new("rikka.fluid.spread", THICKNESS_FORMAT, half),
                false,
            ),
            image(
                graph,
                ImageSpec::new("rikka.fluid.spread", THICKNESS_FORMAT, half),
                false,
            ),
            image(
                graph,
                ImageSpec::new("rikka.fluid.spread", COVERAGE_FORMAT, half),
                false,
            ),
        ],
        coverage: image(
            graph,
            ImageSpec::new("rikka.fluid.coverage", COVERAGE_FORMAT, half),
            true,
        ),
    }
}
