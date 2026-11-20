use rikka_model_core::glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::intensity::Intensity;
use crate::motion::{Automation, HumanMotion};
use crate::part::{Gaze, GazeMode, Goal, Part, PartKey, PartValues, Tag};
use crate::slopes::{Slope, Slopes};

use super::check::{finite, sound, unit};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct HumanFile {
    settings: SettingsFile,
    tracks: Vec<TrackFile>,
}

impl HumanFile {
    pub(super) fn from_motion(motion: &HumanMotion) -> Self {
        Self {
            settings: SettingsFile::from_settings(&motion.settings),
            tracks: Part::ALL
                .into_iter()
                .filter(|part| !motion.parts().keys(part).is_empty())
                .map(|part| TrackFile {
                    part: part.name().to_owned(),
                    keys: motion
                        .parts()
                        .keys(&part)
                        .iter()
                        .map(KeyFile::from_key)
                        .collect(),
                })
                .collect(),
        }
    }

    pub(super) fn motion(self) -> Result<HumanMotion> {
        let mut motion = HumanMotion::default();
        motion.settings = self.settings.automation()?;
        for track in self.tracks {
            let part = Part::named(&track.part).ok_or(Error::UnknownPart(track.part))?;
            for key in track.keys {
                let key = key.key(part)?;
                sound(part.name(), key.frame, is_sound(&key))?;
                if !key.fits() {
                    return Err(Error::InvalidKey {
                        part,
                        frame: key.frame,
                    });
                }
                if motion.parts().key(&part, key.frame).is_some() {
                    return Err(Error::DuplicateKey {
                        part,
                        frame: key.frame,
                    });
                }
                motion.insert(key);
            }
        }
        Ok(motion)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SettingsFile {
    secondary: f32,
    blink: bool,
    limits: bool,
    seed: u64,
    fitting: bool,
    jiggle: f32,
    intensity: IntensityFile,
}

impl SettingsFile {
    fn from_settings(settings: &Automation) -> Self {
        Self {
            secondary: settings.secondary,
            blink: settings.blink,
            limits: settings.limits,
            seed: settings.seed,
            fitting: settings.fitting,
            jiggle: settings.jiggle,
            intensity: IntensityFile::from_intensity(&settings.intensity),
        }
    }

    fn automation(self) -> Result<Automation> {
        let share = |name: &'static str, value: f32| match (0.0..=1.0).contains(&value) {
            true => Ok(value),
            false => Err(Error::OutOfRange(name)),
        };
        Ok(Automation {
            secondary: share("secondary", self.secondary)?,
            blink: self.blink,
            limits: self.limits,
            seed: self.seed,
            fitting: self.fitting,
            jiggle: share("jiggle", self.jiggle)?,
            intensity: self.intensity.intensity()?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct IntensityFile {
    size: f32,
    snap: f32,
    torso: f32,
    head: f32,
    arms: f32,
    legs: f32,
    bounce: f32,
}

impl IntensityFile {
    fn from_intensity(intensity: &Intensity) -> Self {
        Self {
            size: intensity.size,
            snap: intensity.snap,
            torso: intensity.torso,
            head: intensity.head,
            arms: intensity.arms,
            legs: intensity.legs,
            bounce: intensity.bounce,
        }
    }

    fn intensity(self) -> Result<Intensity> {
        let gain = |name: &'static str, value: f32| match Intensity::RANGE.contains(&value) {
            true => Ok(value),
            false => Err(Error::OutOfRange(name)),
        };
        Ok(Intensity {
            size: gain("size", self.size)?,
            snap: gain("snap", self.snap)?,
            torso: gain("torso", self.torso)?,
            head: gain("head", self.head)?,
            arms: gain("arms", self.arms)?,
            legs: gain("legs", self.legs)?,
            bounce: gain("bounce", self.bounce)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TrackFile {
    part: String,
    keys: Vec<KeyFile>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct KeyFile {
    frame: u32,
    tag: String,
    values: ValuesFile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    slopes: Option<SlopesFile>,
}

impl KeyFile {
    fn from_key(key: &PartKey) -> Self {
        Self {
            frame: key.frame,
            tag: key.tag.name().to_owned(),
            values: ValuesFile::from_values(&key.values),
            slopes: key.slopes.as_ref().map(SlopesFile::from_slopes),
        }
    }

    fn key(self, part: Part) -> Result<PartKey> {
        Ok(PartKey {
            part,
            frame: self.frame,
            tag: Tag::named(&self.tag).ok_or(Error::UnknownTag(self.tag))?,
            values: self.values.values()?,
            slopes: self.slopes.map(SlopesFile::slopes),
        })
    }
}

type SlopeFile = [f32; 2];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SlopesFile {
    scalars: Vec<SlopeFile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn: Option<SlopeFile>,
}

impl SlopesFile {
    fn from_slopes(slopes: &Slopes) -> Self {
        let pair = |slope: &Slope| [slope.arriving, slope.leaving];
        Self {
            scalars: slopes.scalars.iter().map(pair).collect(),
            turn: slopes.turn.as_ref().map(pair),
        }
    }

    fn slopes(self) -> Slopes {
        let slope = |[arriving, leaving]: SlopeFile| Slope { arriving, leaving };
        Slopes {
            scalars: self.scalars.into_iter().map(slope).collect(),
            turn: self.turn.map(slope),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct GoalFile {
    position: [f32; 3],
    rotation: [f32; 4],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum ValuesFile {
    Root {
        position: [f32; 3],
        rotation: [f32; 4],
    },
    Muscles(Vec<f32>),
    Limb {
        muscles: Vec<f32>,
        pin: bool,
        goal: GoalFile,
    },
    Gaze {
        eyes: Vec<f32>,
        mode: String,
        target: [f32; 3],
        weight: f32,
        #[serde(rename = "headFollow")]
        head_follow: f32,
    },
}

impl ValuesFile {
    fn from_values(values: &PartValues) -> Self {
        match values {
            PartValues::Root { position, rotation } => Self::Root {
                position: position.to_array(),
                rotation: rotation.to_array(),
            },
            PartValues::Muscles(muscles) => Self::Muscles(muscles.clone()),
            PartValues::Limb { muscles, pin, goal } => Self::Limb {
                muscles: muscles.clone(),
                pin: *pin,
                goal: GoalFile {
                    position: goal.position.to_array(),
                    rotation: goal.rotation.to_array(),
                },
            },
            PartValues::Gaze { eyes, gaze } => Self::Gaze {
                eyes: eyes.clone(),
                mode: gaze.mode.name().to_owned(),
                target: gaze.target.to_array(),
                weight: gaze.weight,
                head_follow: gaze.head_follow,
            },
        }
    }

    fn values(self) -> Result<PartValues> {
        Ok(match self {
            Self::Root { position, rotation } => PartValues::Root {
                position: Vec3::from_array(position),
                rotation: Quat::from_array(rotation),
            },
            Self::Muscles(muscles) => PartValues::Muscles(muscles),
            Self::Limb { muscles, pin, goal } => PartValues::Limb {
                muscles,
                pin,
                goal: Goal {
                    position: Vec3::from_array(goal.position),
                    rotation: Quat::from_array(goal.rotation),
                },
            },
            Self::Gaze {
                eyes,
                mode,
                target,
                weight,
                head_follow,
            } => PartValues::Gaze {
                eyes,
                gaze: Gaze {
                    mode: GazeMode::named(&mode).ok_or(Error::UnknownGaze(mode))?,
                    target: Vec3::from_array(target),
                    weight,
                    head_follow,
                },
            },
        })
    }
}

fn is_sound(key: &PartKey) -> bool {
    let slopes = key.slopes.as_ref().is_none_or(|slopes| {
        slopes
            .scalars
            .iter()
            .chain(&slopes.turn)
            .all(|slope| slope.arriving.is_finite() && slope.leaving.is_finite())
    });
    let values = match &key.values {
        PartValues::Root { position, rotation } => {
            finite(&position.to_array()) && unit(rotation.to_array())
        }
        PartValues::Muscles(muscles) => finite(muscles),
        PartValues::Limb { muscles, goal, .. } => {
            finite(muscles) && finite(&goal.position.to_array()) && unit(goal.rotation.to_array())
        }
        PartValues::Gaze { eyes, gaze } => {
            finite(eyes)
                && finite(&gaze.target.to_array())
                && gaze.weight.is_finite()
                && gaze.head_follow.is_finite()
        }
    };
    slopes && values
}
