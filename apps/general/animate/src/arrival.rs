use std::path::{Path, PathBuf};
use std::sync::Arc;

use animate_body::{Body, Faces};
use animate_cast::{
    Eye, Figure, FigureId, LoadedFigure, LoadedProp, Performer, Prop, PropRole, TakePlayback,
};
use animate_document::{Motion, PropRecord};
use animate_fit::Physique;
use animate_fluid::{Emitter, EmitterId};
use animate_library::Format;
use animate_look::{Look, shared_library};
use animate_motion::{CameraMotion, Clip, Incoming, Merge, Take, Target};
use animate_prop::PropKind;
use animate_transport::Soundtrack;
use animate_wind::WindMotion;
use rikka_model::mmd::{Pmx, Vmd};
use rikka_model::vrm::Vrm;
use rikka_model::{Import, ImportOptions, Model, registry};
use rikka_model_gpu::Prepared;
use rikka_physics_tailor::ClothSettings;
use rikka_renderer::Uploads;

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
    match Format::from_path(path) {
        Some(Format::Pmx | Format::Vrm) => FileKind::Figure,
        Some(
            Format::Fbx | Format::Gltf | Format::Glb | Format::Vrma | Format::Blend | Format::Stl,
        ) => FileKind::Scene,
        Some(Format::Vmd) => FileKind::Motion,
        Some(Format::Motion) => FileKind::RikkaMotion,
        Some(Format::Sound(_)) => FileKind::Sound,
        None => FileKind::Unknown,
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
pub(crate) struct PropTarget {
    pub(crate) role: PropRole,
    pub(crate) model: Arc<Model>,
    pub(crate) faces: Arc<Faces>,
    pub(crate) take: Arc<Take>,
}

impl PropTarget {
    pub(crate) fn from_prop(prop: &Prop) -> Self {
        Self {
            role: prop.role(),
            model: Arc::clone(&prop.model),
            faces: Arc::clone(&prop.faces),
            take: Arc::clone(&prop.take),
        }
    }

    fn target(&self) -> Target<'_> {
        Target {
            model: &self.model,
            body: None,
            faces: &self.faces,
        }
    }
}

#[derive(Debug)]
pub(crate) enum LoadTarget {
    Camera,
    Figure(FigureTarget),
    Prop(PropTarget),
}

impl LoadTarget {
    fn motion_target(&self) -> Option<Target<'_>> {
        match self {
            Self::Camera => None,
            Self::Figure(figure) => Some(figure.target()),
            Self::Prop(prop) => Some(prop.target()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Placement {
    #[default]
    Figure,
    Stage,
    Accessory,
}

impl Placement {
    fn kind(self) -> Option<PropKind> {
        match self {
            Self::Figure => None,
            Self::Stage => Some(PropKind::Stage),
            Self::Accessory => Some(PropKind::Accessory),
        }
    }
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
    pub(crate) placement: Placement,
    pub(crate) target: LoadTarget,
    pub(crate) camera: Option<Arc<CameraMotion>>,
    pub(crate) emitter: Option<EmitterId>,
    pub(crate) uploads: Option<Uploads>,
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
    NoModel,
    NeedsFigure,
    NeedsCamera,
    NeedsStage,
    NeedsAccessory,
    NoSkeleton,
    BodyMismatch,
    Unmatched,
}

#[derive(Debug)]
pub(crate) enum Arrival {
    Figure {
        figure: Figure,
        prepared: Prepared,
    },
    Prop {
        prop: Prop,
        prepared: Prepared,
    },
    Motion {
        figure: FigureId,
        onto: Arc<Take>,
        incoming: Box<Incoming<Take>>,
        take: Arc<Take>,
        playback: Arc<TakePlayback>,
    },
    PropMotion {
        role: PropRole,
        onto: Arc<Take>,
        incoming: Box<Incoming<Take>>,
        take: Arc<Take>,
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
    Placed {
        file: String,
        kind: PropKind,
        record: Box<PropRecord>,
    },
    Choose(Box<Choice>),
    Rejected {
        file: String,
        rejection: Rejection,
    },
}

struct Loader {
    arrivals: Vec<Arrival>,
    placement: Placement,
    target: LoadTarget,
    emitter: Option<EmitterId>,
    posed: Option<Incoming<Take>>,
    filmed: Option<Incoming<CameraMotion>>,
    sound: Option<PathBuf>,
    uploads: Option<Uploads>,
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
        placement: order.placement,
        target: order.target,
        emitter: order.emitter,
        posed: None,
        filmed: None,
        sound: None,
        uploads: order.uploads,
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
        match (sort, self.placement.kind()) {
            (FileKind::Figure | FileKind::Scene, Some(kind)) => match read_model(&path) {
                Ok(model) => self.prop(&path, model, kind),
                Err(error) => self.reject(&path, Rejection::Unreadable(error)),
            },
            (FileKind::Figure, None) => match read_model(&path) {
                Ok(model) => self.figure(&path, model, None),
                Err(error) => self.reject(&path, Rejection::Unreadable(error)),
            },
            (FileKind::Scene, None) => match read_model(&path) {
                Ok(model) => self.scene(path, model),
                Err(error) => self.reject(&path, Rejection::Unreadable(error)),
            },
            (FileKind::Motion, _) => match read_motion(&path) {
                Ok(motion) => self.vmd(&path, motion),
                Err(rejection) => self.reject(&path, rejection),
            },
            (FileKind::RikkaMotion, _) => match read_rikka_motion(&path) {
                Ok(motion) => self.rikka_motion(&path, motion),
                Err(rejection) => self.reject(&path, rejection),
            },
            (FileKind::Sound, _) => self.sound = Some(path),
            (FileKind::Unknown, _) => self.reject(&path, Rejection::Unsupported),
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

    fn prop(&mut self, path: &Path, model: Model, kind: PropKind) {
        if model.meshes.is_empty() {
            return self.reject(path, Rejection::NoModel);
        }
        let name = model_name(&model, path);
        let LoadedProp { prop, prepared } = Prop::load(
            kind,
            name,
            model,
            Some(path.to_path_buf()),
            self.uploads.as_ref(),
        );
        self.target = LoadTarget::Prop(PropTarget::from_prop(&prop));
        self.posed = None;
        self.arrivals.push(Arrival::Prop { prop, prepared });
    }

    fn figure(&mut self, path: &Path, model: Model, clip: Option<usize>) {
        let name = model_name(&model, path);
        let (cloth_settings, sidecar_error) = match ClothSettings::load_for_model(path) {
            Ok(cloth_settings) => (cloth_settings, None),
            Err(error) => (ClothSettings::new(), Some(ReadError::from(error))),
        };
        let loaded: LoadedFigure = Figure::load(
            name,
            model,
            Some(path.to_path_buf()),
            cloth_settings,
            self.uploads.as_ref(),
        );
        self.target = LoadTarget::Figure(FigureTarget::from_figure(&loaded.figure));
        self.posed = None;
        self.arrivals.push(Arrival::Figure {
            figure: loaded.figure,
            prepared: loaded.prepared,
        });
        if loaded.mapping_error.is_some() {
            self.reject(path, Rejection::NoSkeleton);
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
        let clip = match &self.target {
            LoadTarget::Camera => return self.reject(path, Rejection::NeedsFigure),
            LoadTarget::Figure(target) if Arc::ptr_eq(&model, &target.model) => {
                Clip::with_body(model, index, target.body.clone())
            }
            LoadTarget::Figure(_) | LoadTarget::Prop(_) => Clip::new(model, index),
        };
        let (Some(clip), Some(target)) = (clip, self.target.motion_target()) else {
            return self.reject(path, Rejection::NoMotion);
        };
        let arriving = Take::from_clip(target, &clip);
        match arriving.is_still() {
            true => self.reject(path, Rejection::Unmatched),
            false => gather(&mut self.posed, Incoming::merging(arriving)),
        }
    }

    fn vmd(&mut self, path: &Path, motion: Vmd) {
        let (body, lens) = split(motion);
        let moves_figure = !is_empty(&body);
        let moves_camera = !lens.is_empty();
        match (self.target.motion_target(), moves_figure, moves_camera) {
            (None, _, true) => gather(&mut self.filmed, Incoming::merging(lens)),
            (Some(target), true, _) => {
                let arriving = Take::from_vmd(target, body);
                match arriving.is_still() {
                    true => self.reject(path, Rejection::Unmatched),
                    false => gather(&mut self.posed, Incoming::merging(arriving)),
                }
            }
            (None, true, false) => self.reject(path, Rejection::NeedsFigure),
            (Some(_), false, true) => self.reject(path, Rejection::NeedsCamera),
            (_, false, false) => self.reject(path, Rejection::NoMotion),
        }
    }

    fn rikka_motion(&mut self, path: &Path, motion: Motion) {
        match motion {
            Motion::Figure(take) => match &self.target {
                LoadTarget::Figure(target) if target.body.is_none() && take.human.is_some() => {
                    self.reject(path, Rejection::BodyMismatch);
                }
                LoadTarget::Prop(_) if take.human.is_some() => {
                    self.reject(path, Rejection::BodyMismatch);
                }
                LoadTarget::Figure(_) | LoadTarget::Prop(_) => {
                    gather(&mut self.posed, Incoming::replacing(*take));
                }
                LoadTarget::Camera => self.reject(path, Rejection::NeedsFigure),
            },
            Motion::Camera(lens) => match self.target {
                LoadTarget::Camera => gather(&mut self.filmed, Incoming::replacing(*lens)),
                LoadTarget::Figure(_) | LoadTarget::Prop(_) => {
                    self.reject(path, Rejection::NeedsCamera);
                }
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
            Motion::Stage(record) => self.arrivals.push(Arrival::Placed {
                file: file_name(path),
                kind: PropKind::Stage,
                record,
            }),
            Motion::Accessory(record) => self.arrivals.push(Arrival::Placed {
                file: file_name(path),
                kind: PropKind::Accessory,
                record,
            }),
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
        if let Some(incoming) = self.posed.take() {
            match self.target {
                LoadTarget::Figure(FigureTarget {
                    id,
                    model,
                    body,
                    faces,
                    physique,
                    take: onto,
                }) => {
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
                LoadTarget::Prop(PropTarget {
                    role, take: onto, ..
                }) => {
                    let take = Arc::new(incoming.onto(&onto));
                    self.arrivals.push(Arrival::PropMotion {
                        role,
                        onto,
                        incoming: Box::new(incoming),
                        take,
                    });
                }
                LoadTarget::Camera => {}
            }
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

fn model_name(model: &Model, path: &Path) -> String {
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
    let bytes = std::fs::read(path).map_err(|error| Rejection::Unreadable(error.into()))?;
    Vmd::read(&bytes, &ImportOptions::default())
        .map_err(|error| Rejection::Unreadable(error.into()))
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
