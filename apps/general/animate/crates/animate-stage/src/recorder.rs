use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Instant;

use rikka_renderer::{Readback, Renderer};
use rikka_renderer_3d::OffscreenViewport;
use rikka_types::PixelSize;

use super::capture::{SETTLE_LIMIT, WARMUP_RENDERS, still_config};
use super::render::Render;
use super::scene::Scene;
use super::{CaptureView, Replica, StageError};

pub struct Recorder {
    scene: Scene,
    render: Render,
    offscreen: OffscreenViewport,
    shots: u64,
    renderer: Renderer,
    roused: Receiver<()>,
}

impl Recorder {
    pub fn open(
        replica: Replica,
        mut renderer: Renderer,
        size: PixelSize,
    ) -> Result<Self, StageError> {
        let Replica {
            mut input,
            mut render,
        } = replica;
        for member in &input.scene.members {
            member.wait_for_bake();
        }
        let (rouse, roused) = mpsc::channel();
        render.notify_with(Arc::new(move || {
            let _ = rouse.send(());
        }));
        render.sync(&mut input);
        render.prepare(&input.scene);
        render.upload(&mut renderer, &input.scene);
        render.settle_cloth_assets(&input.scene);
        if let Some(error) = render.errors.pop() {
            return Err(error);
        }
        let offscreen = OffscreenViewport::open(&mut renderer, still_config(size))?;
        Ok(Self {
            scene: input.scene,
            render,
            offscreen,
            shots: 0,
            renderer,
            roused,
        })
    }

    pub fn renderer(&self) -> &Renderer {
        &self.renderer
    }

    pub fn readback(&mut self) -> Readback {
        self.renderer.create_readback()
    }

    pub fn render_frame(&mut self, time: f32, into: &mut Readback) -> Result<(), StageError> {
        let mut renders = match self.shots {
            0 => WARMUP_RENDERS,
            _ => 1,
        };
        let deadline = Instant::now() + SETTLE_LIMIT;
        loop {
            self.render.render_still(
                &mut self.renderer,
                &self.scene,
                &mut self.offscreen,
                time,
                CaptureView::CameraMotion,
                renders,
            )?;
            if self.render.dressing.is_settled() {
                break;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            match self.roused.recv_timeout(left) {
                Ok(()) => renders = WARMUP_RENDERS,
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                    tracing::warn!(time, "export frame rendered before its shaders were ready");
                    break;
                }
            }
        }
        if let Some(error) = self.render.errors.pop() {
            return Err(error);
        }
        let target = self.offscreen.present(&mut self.renderer)?;
        self.renderer.start_readback(target, into)?;
        self.shots += 1;
        Ok(())
    }

    pub fn read_back(&mut self, readback: &mut Readback) -> Result<(), StageError> {
        match self.renderer.wait_for_pixels(readback)? {
            true => Ok(()),
            false => Err(StageError::Readback),
        }
    }
}

impl std::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recorder")
            .field("render", &self.render)
            .field("shots", &self.shots)
            .finish_non_exhaustive()
    }
}
