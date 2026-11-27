use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use animate_body::Faces;
use animate_edit::{Key, KeyId, Keyed};
use animate_motion::Take;
use animate_prop::{PropKind, PropMotion};
use animate_sheet::{BoneDisplay, Sheet};
use rikka_model::Model;
use rikka_model::mmd::Pmx;
use rikka_model_gpu::{Prepared, UploadOptions};
use rikka_renderer::Uploads;
use rustc_hash::FxHashSet;

use crate::cast::{Sheets, Subject};
use crate::playback::{Eye, Performer, TakePlayback};

static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PropId(u64);

impl PropId {
    pub fn fresh() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PropRole {
    Stage,
    Accessory(PropId),
}

impl PropRole {
    pub fn of(subject: Subject) -> Option<Self> {
        match subject {
            Subject::Stage => Some(Self::Stage),
            Subject::Accessory(id) => Some(Self::Accessory(id)),
            _ => None,
        }
    }

    pub fn subject(self) -> Subject {
        match self {
            Self::Stage => Subject::Stage,
            Self::Accessory(id) => Subject::Accessory(id),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Prop {
    pub id: PropId,
    pub kind: PropKind,
    pub name: String,
    pub path: Option<PathBuf>,
    pub model: Arc<Model>,
    pub faces: Arc<Faces>,
    pub display: Arc<BoneDisplay>,
    pub motion: PropMotion,
    pub take: Arc<Take>,
}

#[derive(Debug)]
pub struct LoadedProp {
    pub prop: Prop,
    pub prepared: Prepared,
}

impl Prop {
    pub fn load(
        kind: PropKind,
        name: String,
        model: Model,
        path: Option<PathBuf>,
        uploads: Option<&Uploads>,
    ) -> LoadedProp {
        let model = Arc::new(model);
        let display = model
            .extensions
            .get::<Pmx>()
            .map(BoneDisplay::from_pmx)
            .unwrap_or_default();
        let prop = Self {
            id: PropId::fresh(),
            kind,
            name,
            path,
            faces: Arc::new(Faces::from_model(&model)),
            display: Arc::new(display),
            model: Arc::clone(&model),
            motion: PropMotion::default(),
            take: Arc::default(),
        };
        LoadedProp {
            prop,
            prepared: Prepared::with_uploads(model, &UploadOptions::default(), uploads),
        }
    }

    pub fn performer(&self) -> Performer<'_> {
        Performer {
            body: None,
            physique: None,
            faces: &self.faces,
        }
    }

    pub fn last_key(&self) -> u32 {
        self.motion.last_frame().max(self.take.last_frame())
    }

    pub fn is_still(&self) -> bool {
        self.motion.tracks().is_empty() && self.motion.anchors().is_empty() && self.take.is_still()
    }

    pub fn role(&self) -> PropRole {
        match self.kind {
            PropKind::Stage => PropRole::Stage,
            PropKind::Accessory => PropRole::Accessory(self.id),
        }
    }

    pub fn material_class(&self, material: u32) -> Option<&'static str> {
        crate::material::material_class(&self.model, material)
    }
}

impl PartialEq for Prop {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.kind == other.kind
            && self.name == other.name
            && self.path == other.path
            && Arc::ptr_eq(&self.model, &other.model)
            && self.motion == other.motion
            && self.take == other.take
    }
}

impl Keyed for Prop {
    fn gather_keys(&self, ids: &FxHashSet<KeyId>) -> Vec<Key> {
        let mut keys = self.motion.gather_keys(ids);
        keys.extend(self.take.gather_keys(ids));
        keys
    }

    fn remove_keys(&mut self, ids: &FxHashSet<KeyId>) {
        self.motion.remove_keys(ids);
        self.take.remove_keys(ids);
    }

    fn insert_keys(&mut self, keys: &[Key]) {
        self.motion.insert_keys(keys);
        self.take.insert_keys(keys);
    }
}

impl Sheets for Prop {
    fn sheet(&self) -> Sheet {
        Sheet::prop_with_take(
            &self.motion,
            self.kind.attaches(),
            &self.display,
            &self.faces,
            &self.take,
        )
    }
}

#[derive(Clone, Debug)]
pub struct PropTrack {
    pub prop: Arc<Prop>,
    pub sheet: Arc<Sheet>,
    pub playback: Option<Arc<TakePlayback>>,
}

impl PropTrack {
    pub fn new(prop: Arc<Prop>) -> Self {
        let playback = playback_of(&prop);
        Self {
            sheet: Arc::new(prop.sheet()),
            prop,
            playback,
        }
    }

    pub fn revised(&self, prop: Arc<Prop>) -> Self {
        let playback = match Arc::ptr_eq(&self.prop.take, &prop.take)
            && Arc::ptr_eq(&self.prop.model, &prop.model)
        {
            true => self.playback.clone(),
            false => playback_of(&prop),
        };
        Self {
            sheet: Arc::new(prop.sheet()),
            prop,
            playback,
        }
    }
}

fn playback_of(prop: &Prop) -> Option<Arc<TakePlayback>> {
    (!prop.take.is_still()).then(|| {
        Arc::new(TakePlayback::bind(
            &prop.model,
            &prop.take,
            prop.performer(),
            &Eye::default(),
        ))
    })
}
