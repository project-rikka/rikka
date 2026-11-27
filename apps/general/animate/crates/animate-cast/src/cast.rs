use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use animate_body::{Body, Faces, MappingError};
use animate_fit::Physique;
use animate_fluid::{Emitter, EmitterId};
use animate_look::Look;
use animate_motion::{CameraMotion, GazeMode, Part, PartValues, Take};
use animate_sheet::{Listing, Sheet};
use animate_wind::WindMotion;
use rikka_model::Model;
use rikka_model::humanoid::Anatomy;
use rikka_model::mmd::Pmx;
use rikka_model_gpu::{HeadRest, Prepared, UploadOptions};
use rikka_physics_tailor::ClothSettings;
use rikka_renderer::Uploads;

use crate::cloth::Cloth;
use crate::mode::Mode;
use crate::playback::{Performer, TakePlayback};
use crate::prop::{Prop, PropId, PropTrack};
use crate::skeleton::Skeleton;

fn everything(_: &str) -> bool {
    true
}

fn joined<T>(handle: std::thread::ScopedJoinHandle<'_, T>) -> T {
    handle
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FigureId(u64);

impl FigureId {
    pub fn fresh() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    pub fn number(self) -> u64 {
        self.0
    }

    pub fn slot(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Subject {
    Figure(FigureId),
    Stage,
    Accessory(PropId),
    Emitter(EmitterId),
    Camera,
    Look,
    Wind,
}

#[derive(Debug)]
pub struct Figure {
    pub id: FigureId,
    pub name: String,
    pub model: Arc<Model>,
    pub anatomy: Arc<Anatomy>,
    pub skeleton: Arc<Skeleton>,
    pub body: Option<Arc<Body>>,
    pub faces: Arc<Faces>,
    pub head: Option<HeadRest>,
    pub physique: Option<Arc<Physique>>,
    pub take: Arc<Take>,
    pub playback: Option<Arc<TakePlayback>>,
    pub sheet: Arc<Sheet>,
    pub cloth: Cloth,
}

#[derive(Debug)]
pub struct LoadedFigure {
    pub figure: Figure,
    pub prepared: Prepared,
    pub mapping_error: Option<MappingError>,
}

impl Figure {
    pub fn load(
        name: String,
        model: Model,
        path: Option<PathBuf>,
        cloth_settings: ClothSettings,
        uploads: Option<&Uploads>,
    ) -> LoadedFigure {
        let model = Arc::new(model);
        std::thread::scope(|scope| {
            let prepared = scope.spawn(|| {
                Prepared::with_uploads(Arc::clone(&model), &UploadOptions::default(), uploads)
            });
            let faces = scope.spawn(|| Faces::from_model(&model));
            let cloth = scope.spawn(|| Cloth::new(&model, path, cloth_settings));
            let anatomy = Arc::new(Anatomy::recognize(&model));
            let pmx = model.extensions.contains::<Pmx>();
            let (body, mapping_error) = match Body::from_anatomy(&model, &anatomy) {
                Ok(body) => (Some(Arc::new(body)), None),
                Err(error) if pmx || !anatomy.humanoid().is_empty() => (None, Some(error)),
                Err(_) => (None, None),
            };
            let skeleton = Arc::new(Skeleton::new(&model, &anatomy, body.as_deref()));
            let head = HeadRest::new(&model, &anatomy);
            let figure = Self {
                id: FigureId::fresh(),
                name,
                faces: Arc::new(joined(faces)),
                head,
                model: Arc::clone(&model),
                anatomy,
                skeleton,
                body,
                physique: None,
                take: Arc::new(Take::default()),
                playback: None,
                sheet: Arc::default(),
                cloth: joined(cloth),
            };
            LoadedFigure {
                figure,
                prepared: joined(prepared),
                mapping_error,
            }
        })
    }

    pub fn performer(&self) -> Performer<'_> {
        Performer {
            body: self.body.as_ref(),
            physique: self.physique.as_ref(),
            faces: &self.faces,
        }
    }

    pub fn resheet(&mut self, mode: Mode) {
        let undriven = |name: &str| !self.skeleton.is_driven(name);
        let sheet = Sheet::of_take(self.listing(mode, &undriven), &self.faces, &self.take);
        self.sheet = Arc::new(sheet);
    }

    pub fn retake(&mut self, before: &Take, mode: Mode) {
        let undriven = |name: &str| !self.skeleton.is_driven(name);
        let sheet = self.sheet.retaken(
            self.listing(mode, &undriven),
            &self.faces,
            before,
            &self.take,
        );
        self.sheet = Arc::new(sheet);
    }

    fn listing<'a>(&'a self, mode: Mode, undriven: &'a dyn Fn(&str) -> bool) -> Listing<'a> {
        let display = self.skeleton.display();
        match (&self.body, mode) {
            (Some(_), Mode::Easy) => Listing::Parts,
            (Some(_), Mode::Detail) => Listing::Bones {
                display,
                shown: undriven,
            },
            (None, _) if self.skeleton.bones().is_empty() => Listing::Morphs,
            (None, _) => Listing::Bones {
                display,
                shown: &everything,
            },
        }
    }

    pub fn is_human(&self) -> bool {
        self.body.is_some()
    }

    pub fn watches_camera(&self) -> bool {
        self.take.human.as_ref().is_some_and(|human| {
            human.parts().keys(&Part::Gaze).iter().any(|key| {
                matches!(&key.values, PartValues::Gaze { gaze, .. } if gaze.mode == GazeMode::Camera)
            })
        })
    }
}

impl Figure {
    pub fn material_class(&self, material: u32) -> Option<&'static str> {
        crate::material::material_class(&self.model, material)
    }

    pub fn has_motion(&self) -> bool {
        !self.take.is_still()
    }

    pub fn last_key(&self) -> u32 {
        self.take.last_frame()
    }
}

pub trait Sheets {
    fn sheet(&self) -> Sheet;
}

impl Sheets for Look {
    fn sheet(&self) -> Sheet {
        Sheet::look(self)
    }
}

impl Sheets for WindMotion {
    fn sheet(&self) -> Sheet {
        Sheet::wind(self)
    }
}

impl Sheets for CameraMotion {
    fn sheet(&self) -> Sheet {
        Sheet::camera(self)
    }
}

impl Sheets for Emitter {
    fn sheet(&self) -> Sheet {
        Sheet::fluid(self)
    }
}

#[derive(Debug)]
pub struct MotionSheet<M> {
    pub motion: Arc<M>,
    pub sheet: Arc<Sheet>,
}

impl<M: Sheets> MotionSheet<M> {
    pub fn new(motion: Arc<M>) -> Self {
        Self {
            sheet: Arc::new(motion.sheet()),
            motion,
        }
    }
}

impl<M: Sheets + Default> Default for MotionSheet<M> {
    fn default() -> Self {
        Self::new(Arc::default())
    }
}

pub type CameraTrack = MotionSheet<CameraMotion>;

pub type LookTrack = MotionSheet<Look>;

pub type WindTrack = MotionSheet<WindMotion>;

pub type EmitterTrack = MotionSheet<Emitter>;

#[derive(Debug, Default)]
pub struct Cast {
    figures: Vec<Figure>,
    emitters: Vec<EmitterTrack>,
    next_emitter: u32,
    camera: Option<CameraTrack>,
    look: LookTrack,
    wind: WindTrack,
    stage: Option<PropTrack>,
    accessories: Vec<PropTrack>,
}

impl Cast {
    pub fn figures(&self) -> &[Figure] {
        &self.figures
    }

    pub fn figure(&self, id: FigureId) -> Option<&Figure> {
        self.figures.iter().find(|figure| figure.id == id)
    }

    pub fn figure_mut(&mut self, id: FigureId) -> Option<&mut Figure> {
        self.figures.iter_mut().find(|figure| figure.id == id)
    }

    pub fn emitters(&self) -> &[EmitterTrack] {
        &self.emitters
    }

    pub fn emitter(&self, id: EmitterId) -> Option<&Arc<Emitter>> {
        self.emitters
            .iter()
            .find(|track| track.motion.id == id)
            .map(|track| &track.motion)
    }

    pub fn fresh_emitter(&mut self) -> EmitterId {
        let id = EmitterId(self.next_emitter);
        self.next_emitter += 1;
        id
    }

    pub fn place_emitter(&mut self, emitter: Arc<Emitter>) {
        let track = EmitterTrack::new(emitter);
        match self
            .emitters
            .binary_search_by_key(&track.motion.id, |entry| entry.motion.id)
        {
            Ok(at) => self.emitters[at] = track,
            Err(at) => self.emitters.insert(at, track),
        }
    }

    pub fn drop_emitter(&mut self, id: EmitterId) -> Option<Subject> {
        let at = self
            .emitters
            .iter()
            .position(|track| track.motion.id == id)?;
        self.emitters.remove(at);
        let neighbour = self
            .emitters
            .get(at)
            .or_else(|| self.emitters.last())
            .map(|track| Subject::Emitter(track.motion.id));
        let figure = self.figures.last().map(|figure| Subject::Figure(figure.id));
        Some(neighbour.or(figure).unwrap_or(Subject::Camera))
    }

    pub fn stage(&self) -> Option<&PropTrack> {
        self.stage.as_ref()
    }

    pub fn set_stage(&mut self, prop: Option<Arc<Prop>>) {
        self.stage = match (self.stage.take(), prop) {
            (_, None) => None,
            (Some(track), Some(prop)) if track.prop.id == prop.id => Some(track.revised(prop)),
            (_, Some(prop)) => Some(PropTrack::new(prop)),
        };
    }

    pub fn accessories(&self) -> &[PropTrack] {
        &self.accessories
    }

    pub fn accessory(&self, id: PropId) -> Option<&PropTrack> {
        self.accessories.iter().find(|track| track.prop.id == id)
    }

    pub fn place_accessory(&mut self, prop: Arc<Prop>) {
        match self
            .accessories
            .iter_mut()
            .find(|track| track.prop.id == prop.id)
        {
            Some(track) => *track = track.revised(prop),
            None => self.accessories.push(PropTrack::new(prop)),
        }
    }

    pub fn drop_accessory(&mut self, id: PropId) -> Option<Subject> {
        let at = self
            .accessories
            .iter()
            .position(|track| track.prop.id == id)?;
        self.accessories.remove(at);
        let neighbour = self
            .accessories
            .get(at)
            .or_else(|| self.accessories.last())
            .map(|track| Subject::Accessory(track.prop.id));
        Some(neighbour.unwrap_or_else(|| self.fallback()))
    }

    pub fn prop(&self, subject: Subject) -> Option<&PropTrack> {
        match subject {
            Subject::Stage => self.stage.as_ref(),
            Subject::Accessory(id) => self.accessory(id),
            _ => None,
        }
    }

    pub fn props(&self) -> impl Iterator<Item = &PropTrack> {
        self.stage.iter().chain(&self.accessories)
    }

    pub fn fallback(&self) -> Subject {
        let figure = self.figures.last().map(|figure| Subject::Figure(figure.id));
        let stage = self.stage.as_ref().map(|_| Subject::Stage);
        figure.or(stage).unwrap_or(Subject::Camera)
    }

    pub fn camera(&self) -> Option<&CameraTrack> {
        self.camera.as_ref()
    }

    pub fn add_figure(&mut self, figure: Figure) {
        self.figures.push(figure);
    }

    pub fn dismiss(&mut self, id: FigureId) -> Option<Subject> {
        let at = self.figures.iter().position(|figure| figure.id == id)?;
        self.figures.remove(at);
        let neighbour = self
            .figures
            .get(at)
            .or_else(|| self.figures.last())
            .map(|figure| Subject::Figure(figure.id));
        Some(neighbour.unwrap_or_else(|| self.fallback()))
    }

    pub fn resheet(&mut self, mode: Mode) {
        for figure in &mut self.figures {
            figure.resheet(mode);
        }
    }

    pub fn set_camera(&mut self, camera: Option<CameraTrack>) {
        self.camera = camera;
    }

    pub fn subjects(&self) -> Vec<Subject> {
        self.figures
            .iter()
            .map(|figure| Subject::Figure(figure.id))
            .chain(self.stage.iter().map(|_| Subject::Stage))
            .chain(
                self.accessories
                    .iter()
                    .map(|track| Subject::Accessory(track.prop.id)),
            )
            .chain(
                self.emitters
                    .iter()
                    .map(|track| Subject::Emitter(track.motion.id)),
            )
            .chain([Subject::Camera, Subject::Look, Subject::Wind])
            .collect()
    }

    pub fn sheet(&self, subject: Subject) -> Arc<Sheet> {
        match subject {
            Subject::Figure(id) => self
                .figure(id)
                .map_or_else(Arc::default, |figure| Arc::clone(&figure.sheet)),
            Subject::Emitter(id) => self
                .emitters
                .iter()
                .find(|track| track.motion.id == id)
                .map_or_else(Arc::default, |track| Arc::clone(&track.sheet)),
            Subject::Camera => self.camera.as_ref().map_or_else(
                || Arc::new(Sheet::camera(&CameraMotion::default())),
                |camera| Arc::clone(&camera.sheet),
            ),
            Subject::Look => Arc::clone(&self.look.sheet),
            Subject::Wind => Arc::clone(&self.wind.sheet),
            Subject::Stage | Subject::Accessory(_) => self
                .prop(subject)
                .map_or_else(Arc::default, |track| Arc::clone(&track.sheet)),
        }
    }

    pub fn look(&self) -> &LookTrack {
        &self.look
    }

    pub fn set_look(&mut self, look: LookTrack) {
        self.look = look;
    }

    pub fn wind(&self) -> &WindTrack {
        &self.wind
    }

    pub fn set_wind(&mut self, wind: WindTrack) {
        self.wind = wind;
    }

    pub fn last_key(&self) -> u32 {
        self.figures
            .iter()
            .map(Figure::last_key)
            .chain(self.camera.iter().map(|camera| camera.motion.last_frame()))
            .chain(self.emitters.iter().map(|track| track.motion.last_frame()))
            .chain(self.props().map(|track| track.prop.last_key()))
            .chain([self.look.motion.last_frame(), self.wind.motion.last_frame()])
            .max()
            .unwrap_or(0)
    }
}
