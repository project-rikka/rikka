use std::path::Path;
use std::sync::Arc;

use rikka_model::glam::Vec3;
use rikka_model::{Import, Instance, registry};
use rikka_model_gpu::{GpuModel, Restyle};
use rikka_renderer::Renderer;
use rikka_renderer_3d::Result;
use rikka_renderer_3d::Three;
use rikka_renderer_3d::{
    Background, Camera, Frame, Light, Look, ShadowQuality, Viewport, ViewportConfig,
};

use crate::allocations::{Phase, counted};
use crate::studio::EXTENT;

const FRAME_RATE: f32 = 60.0;

pub(crate) struct Figures {
    viewport: Viewport,
    gpu: GpuModel,
    restyle: Option<Restyle>,
    instance: Instance,
    look: Look,
    allocated: u64,
}

fn failed(error: impl std::fmt::Display) -> rikka_renderer::Error {
    rikka_renderer::Error::Invalid(format!("failed to load the benchmark figure: {error}"))
}

impl Figures {
    pub(crate) fn new(
        renderer: &mut Renderer,
        asset: &str,
        preset: Option<&str>,
    ) -> Result<Figures> {
        let path = Path::new(asset);
        let model = Arc::new(Import::open(path, registry()).run().map_err(failed)?);
        let gpu = GpuModel::upload(renderer, model).map_err(failed)?;
        let instance = gpu.instance();
        let restyle = match preset {
            Some(key) => {
                let template = rikka_shader_presets_gpu::template(key)
                    .ok_or_else(|| failed(format!("no prebuilt preset {key}")))?;
                Some(gpu.restyle(renderer, &template).map_err(failed)?)
            }
            None => None,
        };
        let viewport = renderer
            .three()
            .create_viewport(ViewportConfig::new(EXTENT.0, EXTENT.1))?;
        Ok(Figures {
            viewport,
            gpu,
            restyle,
            instance,
            look: Look::default(),
            allocated: 0,
        })
    }

    pub(crate) fn allocated(&self) -> u64 {
        self.allocated
    }

    pub(crate) fn draw(&mut self, renderer: &mut Renderer, at: usize, phase: Phase) -> Result<f32> {
        let time = at as f32 / FRAME_RATE;
        let bounds = self.instance.bounds();
        let orbit = 0.3 * time;
        let eye = bounds.center()
            + Vec3::new(orbit.sin(), 0.35, orbit.cos()).normalize() * bounds.radius() * 2.4;
        let camera = Camera::look_at(
            eye,
            bounds.center(),
            Vec3::Y,
            0.8,
            EXTENT.0 as f32 / EXTENT.1 as f32,
            bounds.radius() * 0.02,
        );
        let mut frame = Frame::new(camera);
        frame
            .background(Background::Sky)
            .time(time)
            .look(self.look.clone())
            .light(
                Light::directional(Vec3::new(-0.5, -0.8, -0.6))
                    .intensity(2.5)
                    .shadow(ShadowQuality::High),
            );
        let drawing = self.gpu.drawing(&self.instance);
        match &self.restyle {
            Some(restyle) => drawing.restyled(restyle).draw_into(&mut frame),
            None => drawing.draw_into(&mut frame),
        }
        let (rendered, allocated) = counted(phase, || {
            renderer.three().render(&mut self.viewport, &frame)
        });
        rendered?;
        self.allocated = allocated;
        Ok((EXTENT.0 * EXTENT.1) as f32)
    }
}
