use rikka_sync::Mutex;
use std::sync::Arc;
use std::time::Instant;

use futures_channel::oneshot;
use rikka_model::glam::Vec3;
use rikka_model::{Instance, Shading};
use rikka_model_gpu::{Error as UploadError, GpuModel, Prepared};
use rikka_renderer::{Renderer, Uploads};
use rikka_renderer_3d::{Frame, Light, Outline, ShadowQuality, ToneMapping};
use rikka_types::PixelSize;
use rikka_viewport::SceneCtx;

use crate::screen::figure::orbit::Orbit;

const LONGEST_STEP: f32 = 0.1;

const TOON_AMBIENT: f32 = 0.1;
const TOON_KEY: f32 = 0.9;
const AMBIENT: f32 = 0.25;
const KEY: f32 = 3.0;
const FILL: f32 = 0.8;

pub(crate) type Shared = Arc<Mutex<Scene>>;

pub(crate) struct Scene {
    loaded: Option<LoadedModel>,
    desired: Option<Prepared>,
    uploaded: Option<oneshot::Sender<()>>,
    animation: Option<usize>,
    playing: bool,
    orbit: Orbit,
    error: Option<UploadError>,
    size: PixelSize,
    drawn_at: Option<Instant>,
    dirty: bool,
    uploads: Option<Uploads>,
}

struct LoadedModel {
    gpu: GpuModel,
    instance: Instance,
    toon: bool,
}

impl Scene {
    pub(crate) fn new() -> Self {
        Self {
            loaded: None,
            desired: None,
            uploaded: None,
            animation: None,
            playing: false,
            orbit: Orbit::default(),
            error: None,
            size: PixelSize::ZERO,
            drawn_at: None,
            dirty: true,
            uploads: None,
        }
    }

    pub(crate) fn show(&mut self, prepared: Prepared) -> oneshot::Receiver<()> {
        let (notify, uploaded) = oneshot::channel();
        self.loaded = None;
        self.desired = Some(prepared);
        self.uploaded = Some(notify);
        self.animation = None;
        self.playing = false;
        self.error = None;
        self.dirty = true;
        uploaded
    }

    pub(crate) fn play(&mut self, animation: usize) {
        self.animation = Some(animation);
        self.playing = true;
        if let Some(loaded) = &mut self.loaded {
            start(&mut loaded.instance, animation);
        }
        self.dirty = true;
    }

    pub(crate) fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
        if let Some(loaded) = &mut self.loaded {
            match playing && loaded.instance.player().duration() > 0.0 {
                true => loaded.instance.player_mut().resume(),
                false => loaded.instance.player_mut().pause(),
            }
        }
        self.dirty = true;
    }

    pub(crate) fn is_playing(&self) -> bool {
        match &self.loaded {
            Some(loaded) => loaded.instance.player().is_playing(),
            None => self.playing,
        }
    }

    pub(crate) fn time(&self) -> f32 {
        self.loaded
            .as_ref()
            .map_or(0.0, |loaded| loaded.instance.player().time())
    }

    pub(crate) fn duration(&self) -> Option<f32> {
        self.loaded
            .as_ref()
            .map(|loaded| loaded.instance.player().duration())
            .filter(|duration| *duration > 0.0)
    }

    pub(crate) fn seek(&mut self, time: f32) {
        let Some(loaded) = &mut self.loaded else {
            return;
        };
        loaded.instance.player_mut().seek(time);
        loaded.instance.advance(0.0);
        self.dirty = true;
    }

    pub(crate) fn turn(&mut self, x: f32, y: f32) {
        self.orbit.turn(x, y);
        self.dirty = true;
    }

    pub(crate) fn pan(&mut self, x: f32, y: f32, height: f32) {
        self.orbit.pan(x, y, height);
        self.dirty = true;
    }

    pub(crate) fn dolly(&mut self, notches: f32) {
        self.orbit.dolly(notches);
        self.dirty = true;
    }

    pub(crate) fn recenter(&mut self) {
        if let Some(loaded) = &self.loaded {
            self.orbit = Orbit::framing(loaded.instance.bounds());
        }
        self.dirty = true;
    }

    pub(crate) fn error(&self) -> Option<&UploadError> {
        self.error.as_ref()
    }

    pub(crate) fn uploads(&self) -> Option<Uploads> {
        self.uploads.clone()
    }

    pub(crate) fn build(&mut self, ctx: SceneCtx<'_>) -> Option<Frame> {
        let aspect = ctx.aspect();
        let SceneCtx {
            renderer,
            width,
            height,
        } = ctx;
        self.uploads.get_or_insert_with(|| renderer.uploads());
        if let Some(prepared) = self.desired.take() {
            self.upload(renderer, prepared);
        }
        let size = PixelSize::new(width, height);
        if self.size != size {
            self.size = size;
            self.dirty = true;
        }
        let elapsed = self.elapsed();

        let Self {
            loaded,
            orbit,
            error,
            dirty,
            ..
        } = self;
        let loaded = loaded.as_mut()?;
        if loaded.instance.player().is_playing() {
            loaded.instance.advance(elapsed);
            *dirty = true;
        }
        if !std::mem::take(dirty) {
            return None;
        }

        if let Err(refused) = loaded.gpu.refresh_materials(renderer, &loaded.instance) {
            *error = Some(refused);
        }
        let mut frame = Frame::new(orbit.camera(aspect));
        light(&mut frame, loaded.toon);
        frame.outline(Outline {
            enabled: false,
            ..Outline::default()
        });
        loaded.gpu.draw(&mut frame, &loaded.instance);
        Some(frame)
    }

    fn upload(&mut self, renderer: &mut Renderer, prepared: Prepared) {
        let toon = prepared
            .model()
            .materials
            .iter()
            .any(|material| matches!(material.shading, Shading::Toon(_)));
        let uploaded = GpuModel::upload_prepared(renderer, prepared);
        if let Some(notify) = self.uploaded.take() {
            let _ = notify.send(());
        }
        let gpu = match uploaded {
            Ok(gpu) => gpu,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let mut instance = gpu.instance();
        if let Some(animation) = self.animation {
            start(&mut instance, animation);
            if !self.playing {
                instance.player_mut().pause();
            }
        }
        self.orbit = Orbit::framing(instance.bounds());
        self.loaded = Some(LoadedModel {
            gpu,
            instance,
            toon,
        });
        self.dirty = true;
    }

    fn elapsed(&mut self) -> f32 {
        let now = Instant::now();
        let elapsed = self
            .drawn_at
            .map_or(0.0, |before| now.duration_since(before).as_secs_f32());
        self.drawn_at = Some(now);
        elapsed.min(LONGEST_STEP)
    }
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("loaded", &self.loaded.is_some())
            .field("animation", &self.animation)
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

fn start(instance: &mut Instance, animation: usize) {
    let Some(animation_id) = instance.model().animation_id(animation) else {
        return;
    };
    instance.play(animation_id);
    instance.advance(0.0);
    if instance.player().duration() <= 0.0 {
        instance.player_mut().pause();
    }
}

fn light(frame: &mut Frame, toon: bool) {
    if toon {
        frame
            .tone_mapping(ToneMapping::Clamp)
            .ambient(Vec3::splat(TOON_AMBIENT))
            .light(
                Light::directional(Vec3::new(-0.35, -0.6, -0.7))
                    .intensity(TOON_KEY)
                    .shadow(ShadowQuality::None),
            );
        return;
    }
    frame
        .ambient(Vec3::splat(AMBIENT))
        .light(
            Light::directional(Vec3::new(-0.4, -0.9, -0.5))
                .intensity(KEY)
                .shadow(ShadowQuality::Medium),
        )
        .light(
            Light::directional(Vec3::new(0.6, -0.2, 0.5))
                .intensity(FILL)
                .shadow(ShadowQuality::None),
        );
}
