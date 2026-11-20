use ash::vk;
use glam::{Mat4, Vec4};

use crate::error::{Error, Result};
use crate::fluid::gpu::{GpuFluid, GpuFluidLayer, GpuFluidMaterial, GpuFluidView};
use crate::fluid::layer::FluidLayer;
use crate::fluid::material::FluidMaterial;
use crate::fluid::style::LiquidStyle;
use crate::frame_data::FluidWrite;
use crate::gpu::NONE_INDEX;
use crate::graph::{FrameGraph, ImageId};
use crate::record::{Chain, Record, SceneImages, Stage};
use crate::viewport::Viewport;
use rikka_gpu::core::frame_ctx::GpuFrameCtx;

const SMOOTHING: f32 = 1.2;
const LONGEST_STEP: f32 = 0.25;
const CORNERS: u32 = 6;

#[derive(Clone, Copy, Debug)]
pub(crate) struct LayerDraw {
    pub vertices: u32,
    pub spray_vertices: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct FluidFrame {
    pub address: vk::DeviceAddress,
    pub layers: Vec<LayerDraw>,
    pub styled: bool,
}

impl Record<'_> {
    pub(crate) fn declare_fluid<'f>(
        &mut self,
        graph: &mut FrameGraph<'f>,
        stage: Stage<'f>,
        scene: &mut SceneImages,
        chain: &mut Chain,
    ) {
        for recipe in self.mesh.look_state.fluid.recipes(&self.mesh.builtin) {
            self.mesh.pipelines.prepare(&recipe);
        }
        let images = super::targets::declare(graph);
        super::surface::declare_surface(graph, scene, images);
        super::composite::declare_composite(graph, scene, images, chain);
        if stage.look.is_some() {
            super::stamp::declare_stamp(graph, scene, images);
        }
        scene.fluid = Some(images);
    }

    pub(crate) fn fluid_prepare(
        &mut self,
        ctx: &mut GpuFrameCtx<'_>,
        viewport: &mut Viewport,
        stage: Stage<'_>,
        scene: &SceneImages,
        raster: Mat4,
    ) -> Result<Option<FluidFrame>> {
        let Stage {
            frame,
            style,
            shadows,
            ..
        } = stage;
        let Some(images) = scene.fluid else {
            return Ok(None);
        };
        let slot = |image: ImageId| viewport.graph.bound(image).sampled;
        let scene_depth = scene.scene_depth.map_or(NONE_INDEX, slot);

        let mut layers = Vec::with_capacity(frame.fluids.len());
        let mut materials = Vec::new();
        let mut draws = Vec::with_capacity(frame.fluids.len());
        for layer in &frame.fluids {
            layers.push(gpu_layer(layer, &mut materials));
            draws.push(LayerDraw {
                vertices: corners(layer.count)?,
                spray_vertices: corners(layer.spray_count())?,
            });
        }

        let extent = viewport.extent();
        let camera = frame.camera.view_projection();
        let views: Vec<GpuFluidView> =
            std::iter::once(view(raster, [extent.width as f32, extent.height as f32]))
                .chain(shadows.iter().map(|shadow| {
                    let tile = shadow.tile as f32;
                    view(shadow.view_projection, [tile, tile])
                }))
                .collect();

        let radius = frame
            .fluids
            .iter()
            .map(|layer| layer.radius)
            .fold(0.0, f32::max);
        let history = &viewport.history;
        let delta_time = history
            .time
            .map_or(0.0, |before| (frame.time - before).clamp(0.0, LONGEST_STEP));
        let header = GpuFluid {
            previous_view_projection: match frame.cut {
                true => camera,
                false => history.view_projection.unwrap_or(camera),
            }
            .to_cols_array(),
            depth: slot(images.depth),
            front: slot(images.front),
            smoothed: slot(images.smoothed[0]),
            thickness: slot(images.thickness),
            scattering: slot(images.scattering),
            coverage: slot(images.coverage),
            scene_depth,
            layer_count: layers.len() as u32,
            view_count: views.len() as u32,
            radius,
            delta_time,
            smoothing: SMOOTHING * radius,
            ..GpuFluid::default()
        };
        let address = self.mesh.frames.write_fluid(
            ctx,
            &FluidWrite {
                header,
                layers: &layers,
                materials: &materials,
                views: &views,
            },
        )?;
        let stylized = style.is_some_and(LiquidStyle::is_stylized);
        let cloudy = materials.iter().any(|material| material.scattering > 0.0);
        Ok(Some(FluidFrame {
            address,
            layers: draws,
            styled: stylized && cloudy,
        }))
    }
}

fn gpu_layer(layer: &FluidLayer, materials: &mut Vec<GpuFluidMaterial>) -> GpuFluidLayer {
    let first = materials.len() as u32;
    match layer.materials.is_empty() {
        true => materials.push(FluidMaterial::default().to_gpu()),
        false => materials.extend(layer.materials.iter().map(|material| material.to_gpu())),
    }
    let spacing = 2.0 * layer.radius;
    GpuFluidLayer {
        particles: layer.particles.address(),
        spray: layer
            .spray
            .as_ref()
            .map_or(0, |(buffer, _)| buffer.address()),
        count: layer.count,
        spray_count: layer.spray_count(),
        material_first: first,
        material_count: materials.len() as u32 - first,
        radius: layer.radius,
        volume: spacing * spacing * spacing,
        _pad: [0; 2],
    }
}

fn corners(count: u32) -> Result<u32> {
    count
        .checked_mul(CORNERS)
        .ok_or_else(|| Error::Invalid(format!("{count} fluid particles exceed the per-draw limit")))
}

fn view(view_projection: Mat4, size: [f32; 2]) -> GpuFluidView {
    GpuFluidView {
        view_projection: view_projection.to_cols_array(),
        inverse: view_projection.inverse().to_cols_array(),
        texel: [2.0 / size[0].max(1.0), 2.0 / size[1].max(1.0)],
        perspective: u32::from(view_projection.row(3) != Vec4::W),
        _pad: 0,
    }
}
