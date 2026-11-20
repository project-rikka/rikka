use std::path::{Path, PathBuf};
use std::sync::Arc;

use animate_body::{Body, Faces};
use animate_cast::{BodyFitError, Eye, Figure, FigureId, LoadedFigure, Performer, TakePlayback};
use animate_document::Motion;
use animate_fit::Physique;
use animate_fluid::{Emitter, EmitterId};
use animate_look::{Look, shared_library};
use animate_motion::{CameraMotion, Clip, Incoming, Merge, Take, Target};
use animate_transport::Soundtrack;
use animate_wind::WindMotion;
use rikka_media_format::Kind;
use rikka_model::mmd::{Pmx, Vmd};
use rikka_model::vrm::Vrm;
use rikka_model::{Import, Model, registry};
use rikka_model_gpu::Prepared;
use rikka_physics_tailor::ClothSettings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileKind {
    Figure,
    Scene,
    Motion,
    RikkaMotion,
    Sound,
    Unknown,
}

fn classify(path: &Path) -> FileKind {
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase());
    match extension.as_deref() {
        Some("pmx" | "vrm") => FileKind::Figure,
        Some("fbx" | "gltf" | "glb" | "vrma") => FileKind::Scene,
        Some("vmd") => FileKind::Motion,
        Some(extension) if extension == Motion::EXTENSION => FileKind::RikkaMotion,
        _ if rikka_media_format::kind(path) == Some(Kind::Sound) => FileKind::Sound,
        _ => FileKind::Unknown,
    }
}

#[derive(Debug)]
pub(crate) struct FigureTarget {
    pub(crate) id: FigureId,
    pub(crate) model: Arc<Model>,
    pub(crate) body: Option<Arc<Body>>,
    pub(crate) faces: Arc<Faces>,
    pub(crate) physique: Option<Arc<Physique>>,
    pub(crate) take: Arc<Take>,
}

impl FigureTarget {
    pub(crate) fn from_figure(figure: &Figure) -> Self {
        Self {
            id: figure.id,
            model: Arc::clone(&figure.model),
            body: figure.body.clone(),
            faces: Arc::clone(&figure.faces),
            physique: figure.physique.clone(),
            take: Arc::clone(&figure.take),
        }
    }

    fn target(&self) -> Target<'_> {
        Target {
            model: &self.model,
            body: self.body.as_deref(),
            faces: &self.faces,
        }
    }
}

#[derive(Debug)]
pub(crate) enum LoadTarget {
    Camera,
    Figure(FigureTarget),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Usage {
    Figure { clip: Option<usize> },
    Motion { clip: usize },
}

#[derive(Debug)]
pub(crate) struct Settled {
    pub(crate) path: PathBuf,
    pub(crate) model: Arc<Model>,
    pub(crate) usage: Usage,
}

#[derive(Debug)]
pub(crate) struct LoadRequest {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) settled: Option<Settled>,
    pub(crate) target: LoadTarget,
    pub(crate) camera: Option<Arc<CameraMotion>>,
    pub(crate) emitter: Option<EmitterId>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ClipSummary {
    pub(crate) name: String,
    pub(crate) seconds: f32,
}

#[derive(Debug)]
pub(crate) struct Choice {
    pub(crate) path: PathBuf,
    pub(crate) model: Arc<Model>,
    pub(crate) clips: Vec<ClipSummary>,
    pub(crate) figure: bool,
}

impl Choice {
    fn new(path: PathBuf, model: Model) -> Self {
        let clips = model
            .animations
            .iter()
            .map(|animation| ClipSummary {
                name: animation.name.clone(),
                seconds: animation.duration(),
            })
            .collect();
        Self {
            path,
            figure: !model.meshes.is_empty(),
            model: Arc::new(model),
            clips,
        }
    }

    pub(crate) fn file(&self) -> String {
        file_name(&self.path)
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ReadError {
    #[error("cannot import the file")]
    Import(#[from] rikka_model::Error),
    #[error("cannot read the file")]
    Io(#[from] std::io::Error),
    #[error("cannot read the cloth settings")]
    ClothSettings(#[from] rikka_physics_tailor::Error),
    #[error("cannot read the Rikka motion")]
    Motion(#[from] animate_document::Error),
    #[error("cannot use the look")]
    Look(#[from] animate_look::Error),
    #[error("cannot read the sound")]
    Sound(#[from] rikka_audio::Error),
}

#[derive(Debug)]
pub(crate) enum Rejection {
    Unreadable(ReadError),
    Unsupported,
    NoMotion,
    NeedsFigure,
    NeedsCamera,
    NoSkeleton,
    BodyMismatch,
    Unmatched,
    BodyFit(BodyFitError),
}

#[derive(Debug)]
pub(crate) enum Arrival {
    Figure {
        figure: Figure,
        prepared: Prepared,
    },
    Motion {
        figure: FigureId,
        onto: Arc<Take>,
        incoming: Box<Incoming<Take>>,
        take: Arc<Take>,
        playback: Arc<TakePlayback>,
    },
    Camera {
        onto: Option<Arc<CameraMotion>>,
        incoming: Box<Incoming<CameraMotion>>,
        motion: Arc<CameraMotion>,
    },
    Sound(Soundtrack),
    Look(Arc<Look>),
    Emitter {
        emitter: Box<Emitter>,
        replacing: Option<EmitterId>,
    },
    Wind(Arc<WindMotion>),
    Choose(Box<Choice>),
    Rejected {
        file: String,
        rejection: Rejection,
    },
}

struct Loader {
    arrivals: Vec<Arrival>,
    target: LoadTarget,
    emitter: Option<EmitterId>,
    posed: Option<Incoming<Take>>,
    filmed: Option<Incoming<CameraMotion>>,
    sound: Option<PathBuf>,
}

pub(crate) fn load(order: LoadRequest) -> Vec<Arrival> {
    let mut sorted: Vec<(FileKind, PathBuf)> = order
        .paths
        .into_iter()
        .map(|path| (classify(&path), path))
        .collect();
    sorted.sort_by_key(|(sort, _)| *sort as u8);

    let mut loader = Loader {
        arrivals: Vec::new(),
        target: order.target,
        emitter: order.emitter,
        posed: None,
        filmed: None,
        sound: None,
    };
    if let Some(settled) = order.settled {
        loader.settle(settled);
    }
    for (sort, path) in sorted {
        loader.arrive(sort, path);
    }
    loader.finish(order.camera)
}

impl Loader {
    fn reject(&mut self, path: &Path, rejection: Rejection) {
        self.arrivals.push(Arrival::Rejected {
            file: file_name(path),
            rejection,
        });
    }

    fn settle(&mut self, settled: Settled) {
        match settled.usage {
            Usage::Figure { clip } => {
                let model = Arc::unwrap_or_clone(settled.model);
                self.figure(&settled.path, model, clip);
            }
            Usage::Motion { clip } => self.clip(&settled.path, settled.model, clip),
        }
    }

    fn arrive(&mut self, sort: FileKind, path: PathBuf) {
        match sort {
            FileKind::Figure => match read_model(&path) {
                Ok(model) => self.figure(&path, model, None),
                Err(error) => self.reject(&path, Rejection::Unreadable(error)),
            },
            FileKind::Scene => match read_model(&path) {
                Ok(model) => self.scene(path, model),
                Err(error) => self.reject(&path, Rejection::Unreadable(error)),
            },
            FileKind::Motion => match read_motion(&path) {
                Ok(motion) => self.vmd(&path, motion),
                Err(rejection) => self.reject(&path, rejection),
            },
            FileKind::RikkaMotion => match read_rikka_motion(&path) {
                Ok(motion) => self.rikka_motion(&path, motion),
                Err(rejection) => self.reject(&path, rejection),
            },
            FileKind::Sound => self.sound = Some(path),
            FileKind::Unknown => self.reject(&path, Rejection::Unsupported),
        }
    }

    fn scene(&mut self, path: PathBuf, model: Model) {
        match (model.meshes.is_empty(), model.animations.len()) {
            (false, 0) => self.figure(&path, model, None),
            (true, 0) => self.reject(&path, Rejection::NoMotion),
            (true, 1) => self.clip(&path, Arc::new(model), 0),
            _ => self
                .arrivals
                .push(Arrival::Choose(Box::new(Choice::new(path, model)))),
        }
    }

    fn figure(&mut self, path: &Path, model: Model, clip: Option<usize>) {
        let name = figure_name(&model, path);
        let (cloth_settings, sidecar_error) = match ClothSettings::load_for_model(path) {
            Ok(cloth_settings) => (cloth_settings, None),
            Err(error) => (ClothSettings::new(), Some(ReadError::from(error))),
        };
        let loaded: LoadedFigure =
            Figure::load(name, model, Some(path.to_path_buf()), cloth_settings);
        self.target = LoadTarget::Figure(FigureTarget::from_figure(&loaded.figure));
        self.posed = None;
        self.arrivals.push(Arrival::Figure {
            figure: loaded.figure,
            prepared: loaded.prepared,
        });
        if loaded.mapping_error.is_some() {
            self.reject(path, Rejection::NoSkeleton);
        }
        if let Some(error) = loaded.body_fit_error {
            self.reject(path, Rejection::BodyFit(error));
        }
        if let Some(error) = sidecar_error {
            self.reject(
                &ClothSettings::sidecar_path(path),
                Rejection::Unreadable(error),
            );
        }
        if let (Some(clip), LoadTarget::Figure(target)) = (clip, &self.target) {
            let model = Arc::clone(&target.model);
            self.clip(path, model, clip);
        }
    }

    fn clip(&mut self, path: &Path, model: Arc<Model>, index: usize) {
        let LoadTarget::Figure(target) = &self.target else {
            return self.reject(path, Rejection::NeedsFigure);
        };
        let clip = match Arc::ptr_eq(&model, &target.model) {
            true => Clip::with_body(model, index, target.body.clone()),
            false => Clip::new(model, index),
        };
        let Some(clip) = clip else {
            return self.reject(path, Rejection::NoMotion);
        };
        let arriving = Take::from_clip(target.target(), &clip);
        match arriving.is_still() {
            true => self.reject(path, Rejection::Unmatched),
            false => gather(&mut self.posed, Incoming::merging(arriving)),
        }
    }

    fn vmd(&mut self, path: &Path, motion: Vmd) {
        let (body, lens) = split(motion);
        let moves_figure = !is_empty(&body);
        let moves_camera = !lens.is_empty();
        match (&self.target, moves_figure, moves_camera) {
            (LoadTarget::Camera, _, true) => gather(&mut self.filmed, Incoming::merging(lens)),
            (LoadTarget::Figure(target), true, _) => {
                let arriving = Take::from_vmd(target.target(), body);
                match arriving.is_still() {
                    true => self.reject(path, Rejection::Unmatched),
                    false => gather(&mut self.posed, Incoming::merging(arriving)),
                }
            }
            (LoadTarget::Camera, true, false) => self.reject(path, Rejection::NeedsFigure),
            (LoadTarget::Figure(_), false, true) => self.reject(path, Rejection::NeedsCamera),
            (_, false, false) => self.reject(path, Rejection::NoMotion),
        }
    }

    fn rikka_motion(&mut self, path: &Path, motion: Motion) {
        match motion {
            Motion::Figure(take) => match &self.target {
                LoadTarget::Figure(target) if target.body.is_none() && take.human.is_some() => {
                    self.reject(path, Rejection::BodyMismatch);
                }
                LoadTarget::Figure(_) => gather(&mut self.posed, Incoming::replacing(*take)),
                LoadTarget::Camera => self.reject(path, Rejection::NeedsFigure),
            },
            Motion::Camera(lens) => match self.target {
                LoadTarget::Camera => gather(&mut self.filmed, Incoming::replacing(*lens)),
                LoadTarget::Figure(_) => self.reject(path, Rejection::NeedsCamera),
            },
            Motion::Look(look) => match settle_look(*look, path) {
                Ok(look) => self.arrivals.push(Arrival::Look(Arc::new(look))),
                Err(error) => self.reject(path, Rejection::Unreadable(error.into())),
            },
            Motion::Fluid(loaded) => self.arrivals.push(Arrival::Emitter {
                emitter: loaded,
                replacing: self.emitter.take(),
            }),
            Motion::Wind(wind) => self.arrivals.push(Arrival::Wind(Arc::new(*wind))),
        }
    }

    fn finish(mut self, camera: Option<Arc<CameraMotion>>) -> Vec<Arrival> {
        let filmed = self.filmed.take().map(|incoming| {
            let motion = Arc::new(incoming.onto(&camera.as_deref().cloned().unwrap_or_default()));
            (incoming, motion)
        });
        let eye = Eye::new(
            filmed
                .as_ref()
                .map(|(_, motion)| motion)
                .or(camera.as_ref())
                .cloned(),
        );
        if let Some(incoming) = self.posed.take()
            && let LoadTarget::Figure(FigureTarget {
                id,
                model,
                body,
                faces,
                physique,
                take: onto,
            }) = self.target
        {
            let performer = Performer {
                body: body.as_ref(),
                physique: physique.as_ref(),
                faces: &faces,
            };
            let take = Arc::new(incoming.onto(&onto));
            self.arrivals.push(Arrival::Motion {
                figure: id,
                playback: Arc::new(TakePlayback::bind(&model, &take, performer, &eye)),
                onto,
                incoming: Box::new(incoming),
                take,
            });
        }
        if let Some((incoming, motion)) = filmed {
            self.arrivals.push(Arrival::Camera {
                onto: camera,
                incoming: Box::new(incoming),
                motion,
            });
        }
        if let Some(path) = self.sound {
            self.arrivals.push(match Soundtrack::read(&path) {
                Ok(sound) => Arrival::Sound(sound),
                Err(error) => Arrival::Rejected {
                    file: file_name(&path),
                    rejection: Rejection::Unreadable(error.into()),
                },
            });
        }
        self.arrivals
    }
}

fn gather<M: Merge>(gathered: &mut Option<Incoming<M>>, next: Incoming<M>) {
    match gathered {
        Some(incoming) => incoming.then(next),
        None => *gathered = Some(next),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn read_model(path: &Path) -> Result<Model, ReadError> {
    Ok(Import::open(path, registry()).run()?)
}

fn figure_name(model: &Model, path: &Path) -> String {
    let declared = model
        .extensions
        .get::<Pmx>()
        .map(|pmx| pmx.name.as_str())
        .or_else(|| {
            model
                .extensions
                .get::<Vrm>()
                .map(|vrm| vrm.meta.name.as_str())
        })
        .map(str::trim)
        .filter(|name| !name.is_empty());
    match declared {
        Some(name) => name.to_owned(),
        None => path
            .file_stem()
            .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned()),
    }
}

fn read_motion(path: &Path) -> Result<Vmd, Rejection> {
    let mut model = Import::open(path, registry())
        .run()
        .map_err(|error| Rejection::Unreadable(error.into()))?;
    model
        .extensions
        .remove::<Vmd>()
        .ok_or(Rejection::Unsupported)
}

fn read_rikka_motion(path: &Path) -> Result<Motion, Rejection> {
    Motion::load(path).map_err(|error| Rejection::Unreadable(error.into()))
}

fn settle_look(mut look: Look, path: &Path) -> Result<Look, animate_look::Error> {
    let folder = path.parent().unwrap_or(Path::new(""));
    look.map_assets(shared_library(), |asset| {
        match Path::new(asset).is_relative() {
            true => folder.join(asset).to_string_lossy().into_owned(),
            false => asset.to_owned(),
        }
    })?;
    Ok(look)
}

fn split(motion: Vmd) -> (Vmd, CameraMotion) {
    let Vmd {
        model,
        bones,
        morphs,
        cameras,
        properties,
        ..
    } = motion;
    (
        Vmd {
            model,
            bones,
            morphs,
            properties,
            ..Vmd::default()
        },
        cameras.into_iter().collect(),
    )
}

fn is_empty(motion: &Vmd) -> bool {
    motion.bones.is_empty() && motion.morphs.is_empty() && motion.properties.is_empty()
}
