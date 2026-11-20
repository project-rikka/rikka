mod check;
mod detail;
mod human;

use animate_hold::HoldFile;
use animate_track::{Keyframe, Tracks};
use serde::{Deserialize, Serialize};

use crate::camera::CameraMotion;
use crate::error::{Error, Result};
use crate::take::Take;

use detail::{CameraKeyFile, DetailFile, MorphFile};
use human::HumanFile;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TakeFile {
    human: Option<HumanFile>,
    detail: DetailFile,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    morphs: Vec<MorphFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    holds: Vec<HoldFile>,
}

impl TakeFile {
    #[must_use]
    pub fn from_take(take: &Take) -> Self {
        Self {
            human: take.human.as_ref().map(HumanFile::from_motion),
            detail: DetailFile::from_take(take),
            morphs: take.morphs.all_keys().map(MorphFile::from_key).collect(),
            holds: HoldFile::from_holds(&take.holds),
        }
    }

    pub fn take(self) -> Result<Take> {
        let (bones, switches) = self.detail.tracks()?;
        Ok(Take {
            human: self.human.map(HumanFile::motion).transpose()?,
            bones,
            switches,
            morphs: tracks_of(self.morphs.into_iter().map(MorphFile::key), |key| {
                key.morph.clone()
            })?,
            holds: HoldFile::holds(&self.holds)?,
        })
    }
}

fn tracks_of<K: Keyframe>(
    keys: impl IntoIterator<Item = Result<K>>,
    track: impl Fn(&K) -> String,
) -> Result<Tracks<K>> {
    let mut tracks = Tracks::default();
    for key in keys {
        if let Some(repeated) = tracks.insert(key?) {
            return Err(Error::Repeated {
                track: track(&repeated),
                frame: repeated.frame(),
            });
        }
    }
    Ok(tracks)
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraFile {
    cameras: Vec<CameraKeyFile>,
}

impl CameraFile {
    #[must_use]
    pub fn from_motion(motion: &CameraMotion) -> Self {
        Self {
            cameras: motion
                .keys()
                .iter()
                .map(CameraKeyFile::from_frame)
                .collect(),
        }
    }

    pub fn motion(&self) -> Result<CameraMotion> {
        let keys = tracks_of(self.cameras.iter().map(CameraKeyFile::frame), |_| {
            detail::CAMERA.to_owned()
        })?;
        Ok(keys.all_keys().copied().collect())
    }
}
