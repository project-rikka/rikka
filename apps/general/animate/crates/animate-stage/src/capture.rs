use std::time::{Duration, Instant};

use animate_cast::{Shot, time_of};
use rikka_renderer::Renderer;
use rikka_renderer_3d::{OffscreenViewport, Three, ViewportConfig};
use rikka_types::PixelSize;
use tokio::sync::oneshot;

use crate::StageError;
use crate::render::Render;
use crate::scene::Scene;

pub(super) const WARMUP_RENDERS: usize = 4;
pub(super) const SETTLE_LIMIT: Duration = Duration::from_secs(60);

pub(super) fn still_config(size: PixelSize) -> ViewportConfig {
    ViewportConfig::new(size.width, size.height).identity(false)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CaptureView {
    CameraMotion,
    Free,
    Shot(Shot),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub frame: u32,
    pub rgba: Vec<u8>,
}

pub type CaptureResult = Result<Vec<Picture>, StageError>;

#[derive(Debug)]
pub struct Capture {
    pub frames: Vec<u32>,
    pub view: CaptureView,
    pub size: PixelSize,
    pub done: oneshot::Sender<CaptureResult>,
}

#[derive(Debug)]
pub(super) struct PendingCapture {
    capture: Capture,
    offscreen: Option<OffscreenViewport>,
    pictures: Vec<Picture>,
    started: Instant,
}

impl PendingCapture {
    pub(super) fn new(capture: Capture) -> Self {
        Self {
            capture,
            offscreen: None,
            pictures: Vec::new(),
            started: Instant::now(),
        }
    }
}

impl Render {
    pub(super) fn render_captures(&mut self, renderer: &mut Renderer, scene: &Scene) {
        self.captures
            .retain(|pending| !pending.capture.done.is_closed());
        let Some(mut pending) = self.captures.pop_front() else {
            return;
        };
        self.posed_at = None;
        self.dirty = true;
        match self.render_next_frame(renderer, scene, &mut pending) {
            Ok(false) => self.captures.push_front(pending),
            Ok(true) => {
                let _ = pending.capture.done.send(Ok(pending.pictures));
            }
            Err(error) => {
                let _ = pending.capture.done.send(Err(error));
            }
        }
    }

    fn render_next_frame(
        &mut self,
        renderer: &mut Renderer,
        scene: &Scene,
        pending: &mut PendingCapture,
    ) -> Result<bool, StageError> {
        let capture = &pending.capture;
        let Some(frame) = capture.frames.get(pending.pictures.len()).copied() else {
            return Ok(true);
        };
        let offscreen = match &mut pending.offscreen {
            Some(offscreen) => offscreen,
            offscreen @ None => offscreen.insert(OffscreenViewport::open(
                renderer,
                still_config(capture.size),
            )?),
        };
        self.render_still(
            renderer,
            scene,
            offscreen,
            time_of(frame),
            capture.view,
            WARMUP_RENDERS,
        )?;
        if !self.dressing.is_settled() && pending.started.elapsed() < SETTLE_LIMIT {
            return Ok(false);
        }
        let rgba = offscreen.read_pixels(renderer)?;
        pending.pictures.push(Picture { frame, rgba });
        pending.started = Instant::now();
        Ok(pending.pictures.len() == capture.frames.len())
    }

    pub(super) fn render_still(
        &mut self,
        renderer: &mut Renderer,
        scene: &Scene,
        offscreen: &mut OffscreenViewport,
        time: f32,
        view: CaptureView,
        renders: usize,
    ) -> Result<(), StageError> {
        self.pose_at(renderer, scene, time);
        let shot = match view {
            CaptureView::CameraMotion => scene
                .camera
                .as_ref()
                .and_then(|camera| camera.pose_at(time))
                .map_or(scene.free, Shot::from),
            CaptureView::Free => scene.free,
            CaptureView::Shot(shot) => shot,
        };
        let camera = shot.camera(offscreen.aspect());
        for _ in 0..renders {
            let composed = self.compose(renderer, scene, camera, time);
            renderer
                .three()
                .render(offscreen.viewport_mut(), &composed)?;
        }
        Ok(())
    }
}
