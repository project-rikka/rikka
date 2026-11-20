use ash::vk;

use crate::gpu::NONE_INDEX;
use crate::graph::{FrameGraph, ImageId, ImageSpec, Size, scaled};
use crate::record::stage::SkyImages;

pub(super) const TRANSMITTANCE_SIZE: (u32, u32) = (256, 64);
pub(super) const MULTISCATTER_SIZE: (u32, u32) = (32, 32);
pub(super) const SKY_VIEW_SIZE: (u32, u32) = (256, 128);
pub(super) const ENVIRONMENT_SIZE: u32 = 128;
pub(super) const ENVIRONMENT_LEVELS: u32 = 6;
pub(super) const PANORAMA_SIZE: (u32, u32) = (512, 256);
pub(super) const AERIAL_CELLS: u32 = 32;
pub(super) const AERIAL_SLICES: u32 = 32;
pub(super) const AERIAL_COLUMNS: u32 = 8;
pub(super) const FOG_TILE: u32 = 12;
pub(super) const FOG_SLICES: u32 = 64;
pub(super) const FOG_COLUMNS: u32 = 8;
pub(super) const SHAFTS_SCALE: u32 = 4;

const FORMAT: vk::Format = vk::Format::R16G16B16A16_SFLOAT;

fn table(graph: &mut FrameGraph<'_>, name: &'static str, (width, height): (u32, u32)) -> ImageId {
    let image = graph.create(ImageSpec::new(name, FORMAT, Size::Fixed(width, height)).shared());
    graph.publish(image);
    image
}

pub(super) const TRANSMITTANCE: &str = "rikka.three.sky.transmittance";
pub(super) const MULTISCATTER: &str = "rikka.three.sky.multiscatter";
pub(super) const ENVIRONMENT: &str = "rikka.three.sky.environment";
pub(super) const PANORAMA: &str = "rikka.three.sky.panorama";

pub(super) fn shared(graph: &mut FrameGraph<'_>) -> SkyImages {
    SkyImages {
        transmittance: table(graph, TRANSMITTANCE, TRANSMITTANCE_SIZE),
        multiscatter: table(graph, MULTISCATTER, MULTISCATTER_SIZE),
        view: table(graph, "rikka.three.sky.view", SKY_VIEW_SIZE),
        environment_base: table(
            graph,
            "rikka.three.sky.environment_base",
            (ENVIRONMENT_SIZE, ENVIRONMENT_SIZE),
        ),
        environment: table(
            graph,
            ENVIRONMENT,
            (ENVIRONMENT_SIZE + ENVIRONMENT_SIZE / 2, ENVIRONMENT_SIZE),
        ),
        aerial: None,
        levels: ENVIRONMENT_LEVELS,
        panorama: NONE_INDEX,
    }
}

pub(super) fn panorama(graph: &mut FrameGraph<'_>) -> ImageId {
    graph.create(
        ImageSpec::new(
            PANORAMA,
            FORMAT,
            Size::Fixed(PANORAMA_SIZE.0, PANORAMA_SIZE.1 + PANORAMA_SIZE.1 / 2),
        )
        .shared(),
    )
}

pub(super) fn aerial(graph: &mut FrameGraph<'_>) -> ImageId {
    let image = graph.create(ImageSpec::new(
        "rikka.three.sky.aerial",
        FORMAT,
        Size::Fixed(
            AERIAL_CELLS * AERIAL_COLUMNS,
            AERIAL_CELLS * (AERIAL_SLICES / AERIAL_COLUMNS),
        ),
    ));
    graph.publish(image);
    image
}

pub(super) fn fog(graph: &mut FrameGraph<'_>) -> (ImageId, [u32; 3]) {
    let grid = fog_grid(graph.extent());
    let room = fog_grid(graph.capacity());
    let image = graph.create(ImageSpec::new(
        "rikka.three.sky.fog",
        FORMAT,
        Size::Fixed(room.0 * FOG_COLUMNS, room.1 * (FOG_SLICES / FOG_COLUMNS)),
    ));
    graph.publish(image);
    (image, [grid.0, grid.1, FOG_SLICES])
}

pub(super) fn shafts(graph: &mut FrameGraph<'_>) -> ImageId {
    graph.create(ImageSpec::new(
        "rikka.three.sky.shafts",
        vk::Format::R32_SFLOAT,
        Size::Divided(SHAFTS_SCALE),
    ))
}

pub(super) fn fog_grid(extent: vk::Extent2D) -> (u32, u32) {
    (
        extent.width.div_ceil(FOG_TILE).max(1),
        extent.height.div_ceil(FOG_TILE).max(1),
    )
}

pub(super) fn shafts_size(extent: vk::Extent2D) -> (u32, u32) {
    let size = scaled(extent, SHAFTS_SCALE);
    (size.width, size.height)
}
