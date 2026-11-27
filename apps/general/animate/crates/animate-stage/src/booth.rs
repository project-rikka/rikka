use std::sync::Arc;

use animate_body::{Body, MorphWeights};
use animate_cast::{FigureId, Shot};
use animate_look::{Look, Name};
use animate_shader::{SharedCompiler, SharedShaders};
use rikka_model::Instance;
use rikka_model_gpu::{GpuModel, HeadRest, Prepared};
use rikka_renderer::Renderer;
use rikka_renderer_3d::{DEFAULT_LIGHT_CHANNELS, Frame, OffscreenViewport, Outline, Three};
use rikka_types::PixelSize;
use rustc_hash::FxHashMap;

use crate::actor::perform;
use crate::capture::{WARMUP_RENDERS, still_config};
use crate::dressing::{Dressing, Moment, Player};
use crate::{Notify, StageError};

const SITTER: &str = "Sitter";
const SITTER_CHANNEL: u32 = 1 << 1;

pub struct Studio {
    dressing: Dressing,
    look: Arc<Look>,
    gpus: FxHashMap<FigureId, GpuModel>,
}

#[derive(Clone, Copy, Debug)]
pub struct Sitting<'a> {
    pub figure: FigureId,
    pub instance: &'a Instance,
    pub head: Option<&'a HeadRest>,
}

impl Studio {
    pub fn new(shaders: SharedShaders, compiler: SharedCompiler, look: Arc<Look>) -> Self {
        Self {
            dressing: Dressing::new(shaders, compiler),
            look,
            gpus: FxHashMap::default(),
        }
    }

    pub fn notify_with(&mut self, notify: Notify) {
        self.dressing.notify_with(notify);
    }

    pub fn seat(
        &mut self,
        renderer: &mut Renderer,
        figure: FigureId,
        prepared: Prepared,
    ) -> Result<Instance, StageError> {
        let gpu = GpuModel::upload_prepared(renderer, prepared).map_err(StageError::Upload)?;
        let instance = gpu.instance();
        self.gpus.insert(figure, gpu);
        Ok(instance)
    }

    pub fn dismiss(&mut self, figure: FigureId) {
        self.gpus.remove(&figure);
        self.dressing.forget(figure);
    }

    pub fn is_settled(&self) -> bool {
        self.dressing.is_settled()
    }

    pub fn still(
        &mut self,
        renderer: &mut Renderer,
        offscreen: &mut OffscreenViewport,
        shot: Shot,
        sitting: Sitting<'_>,
    ) -> Result<(), StageError> {
        for _ in 0..WARMUP_RENDERS {
            self.frame(renderer, offscreen, shot, sitting)?;
        }
        Ok(())
    }

    pub fn frame(
        &mut self,
        renderer: &mut Renderer,
        offscreen: &mut OffscreenViewport,
        shot: Shot,
        sitting: Sitting<'_>,
    ) -> Result<(), StageError> {
        let camera = shot.camera(offscreen.aspect());
        let mut frame = Frame::new(camera);
        frame.outline(Outline {
            enabled: false,
            ..Outline::default()
        });
        let players = [Player {
            id: sitting.figure,
            name: SITTER,
            look_name: Name::new(SITTER),
            head: sitting.head.map(|head| head.place(sitting.instance)),
            instance: sitting.instance,
            focused: true,
            channel: SITTER_CHANNEL,
        }];
        let staged = self.dressing.stage(
            renderer,
            &mut frame,
            &self.look,
            Moment {
                camera: &camera,
                time: 0.0,
                players: &players,
            },
            &mut self.gpus,
        );
        if let Some(gpu) = self.gpus.get(&sitting.figure) {
            gpu.refresh_materials(renderer, sitting.instance)
                .map_err(StageError::Upload)?;
            let drawing = gpu
                .drawing(sitting.instance)
                .light_channels(DEFAULT_LIGHT_CHANNELS | SITTER_CHANNEL);
            match staged.figures.get(&sitting.figure) {
                Some(figure) => drawing.figure(*figure).draw_into(&mut frame),
                None => drawing.draw_into(&mut frame),
            }
        }
        if let Some(reason) = self.dressing.take_failure() {
            return Err(StageError::Look(reason));
        }
        renderer.three().render(offscreen.viewport_mut(), &frame)?;
        Ok(())
    }
}

impl std::fmt::Debug for Studio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Studio")
            .field("dressing", &self.dressing)
            .field("seated", &self.gpus.len())
            .finish_non_exhaustive()
    }
}

pub fn still_viewport(
    renderer: &mut Renderer,
    size: PixelSize,
) -> Result<OffscreenViewport, StageError> {
    Ok(OffscreenViewport::open(renderer, still_config(size))?)
}

pub fn rest(instance: &mut Instance, body: Option<&Body>) {
    perform(None, body, &MorphWeights::default(), instance, 0.0);
    instance.refresh();
}
