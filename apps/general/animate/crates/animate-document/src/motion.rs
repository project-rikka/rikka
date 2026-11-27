use std::path::Path;

use animate_fluid::{Emitter, EmitterFile, EmitterId};
use animate_look::{Look, LookFile, shared_library};
use animate_motion::{CameraFile, CameraMotion, Take, TakeFile};
use animate_wind::{WindFile, WindMotion};
use rikka_file_format::JsonFormat;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::prop::{PropBody, PropRecord};

const FORMAT: JsonFormat = JsonFormat::new("rikka-motion");

#[derive(Clone, Debug, PartialEq)]
pub enum Motion {
    Figure(Box<Take>),
    Camera(Box<CameraMotion>),
    Look(Box<Look>),
    Fluid(Box<Emitter>),
    Wind(Box<WindMotion>),
    Stage(Box<PropRecord>),
    Accessory(Box<PropRecord>),
}

impl Motion {
    pub const EXTENSION: &'static str = "rmotion";
    pub const MIME: &'static str = "model/x.rikka-motion";

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        Ok(FORMAT.save(path, &Body::from_motion(self))?)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        FORMAT.load::<Body>(path)?.motion()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Body {
    Figure(TakeFile),
    Camera(CameraFile),
    Look(LookFile),
    Fluid(EmitterFile),
    Wind(WindFile),
    Stage(PropBody),
    Accessory(PropBody),
}

impl Body {
    fn from_motion(motion: &Motion) -> Self {
        match motion {
            Motion::Figure(take) => Self::Figure(TakeFile::from_take(take)),
            Motion::Camera(camera) => Self::Camera(CameraFile::from_motion(camera)),
            Motion::Look(look) => Self::Look(LookFile::from_look(look)),
            Motion::Fluid(emitter) => Self::Fluid(EmitterFile::from_emitter(emitter)),
            Motion::Wind(wind) => Self::Wind(WindFile::from_motion(wind)),
            Motion::Stage(record) => Self::Stage(PropBody::from_record(record)),
            Motion::Accessory(record) => Self::Accessory(PropBody::from_record(record)),
        }
    }

    fn motion(self) -> Result<Motion> {
        Ok(match self {
            Self::Figure(file) => Motion::Figure(Box::new(file.take()?)),
            Self::Camera(file) => Motion::Camera(Box::new(file.motion()?)),
            Self::Look(file) => Motion::Look(Box::new(file.look(shared_library())?)),
            Self::Fluid(file) => Motion::Fluid(Box::new(file.emitter(EmitterId(0))?)),
            Self::Wind(file) => Motion::Wind(Box::new(file.motion()?)),
            Self::Stage(body) => Motion::Stage(Box::new(body.record()?)),
            Self::Accessory(body) => Motion::Accessory(Box::new(body.record()?)),
        })
    }
}
