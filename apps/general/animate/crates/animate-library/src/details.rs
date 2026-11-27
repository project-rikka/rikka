use std::path::Path;
use std::sync::Arc;

use rikka_model::vrm::{LegacyLicense, Meta, VrmVersion};

use crate::format::Format;
use crate::probe;

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Details {
    #[default]
    Pending,
    Model(Box<ModelDetails>),
    Motion(Box<MotionDetails>),
    Sound(Box<SoundDetails>),
    Unreadable(Arc<str>),
}

impl Details {
    #[must_use]
    pub fn read(path: &Path, format: Format) -> Details {
        probe::read(path, format, &rikka_types::CancelToken::new())
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        matches!(self, Details::Pending)
    }

    #[must_use]
    pub fn has_embedded_picture(&self) -> bool {
        self.embedded().is_some()
    }

    pub(crate) fn embedded(&self) -> Option<Embedded> {
        match self {
            Details::Model(model) => model.embedded,
            Details::Sound(sound) if sound.cover => Some(Embedded::Cover),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelDetails {
    pub title: Option<String>,
    pub author: Option<String>,
    pub comment: Option<String>,
    pub vertices: Option<u64>,
    pub materials: Option<u32>,
    pub meshes: Option<u32>,
    pub clips: Vec<Clip>,
    pub vrm: Option<VrmDetails>,
    pub(crate) embedded: Option<Embedded>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VrmDetails {
    pub version: VrmVersion,
    pub meta: Meta,
    pub legacy: Option<LegacyLicense>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub name: String,
    pub seconds: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentKind {
    Figure,
    Camera,
    Look,
    Fluid,
    Wind,
    Stage,
    Accessory,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MotionDetails {
    pub seconds: f32,
    pub frames: Option<u32>,
    pub body: bool,
    pub morphs: bool,
    pub camera: bool,
    pub target: Option<String>,
    pub clips: Vec<Clip>,
    pub document: Option<DocumentKind>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SoundDetails {
    pub seconds: f32,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub cover: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Embedded {
    Range { offset: u64, length: u64 },
    Cover,
}
