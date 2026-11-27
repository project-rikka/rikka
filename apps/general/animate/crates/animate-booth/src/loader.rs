use std::path::Path;
use std::sync::{Arc, LazyLock};

use animate_body::{Body, Dof, Faces, HandPreset, HumanBone, HumanPose, Side, muscle_of, muscles};
use animate_cast::Shot;
use animate_document::Motion;
use animate_motion::{CameraMotion, Clip, Recipient, Retarget};
use rikka_model::glam::Vec3;
use rikka_model::humanoid::Anatomy;
use rikka_model::mmd::{FRAMES_PER_SECOND, Vmd};
use rikka_model::{Import, ImportOptions, Instance, Limit, Model, registry};
use rikka_sync::Mutex;

use crate::error::BoothError;
use crate::framing::{Aim, Fit};
use crate::memo::Memo;
use crate::moment::{camera_frame, expressive_frame};
use crate::setup::{Facts, Performance, Posture, Setup, View};
use crate::sitter::{ModelKey, Performer, Sitter, Source, Stamp};

const PORTRAIT_TURN: f32 = 25.0;
const PORTRAIT_ELEVATION: f32 = 6.0;
const OBJECT_TURN: f32 = 30.0;
const OBJECT_ELEVATION: f32 = 20.0;
const STAGE_TURN: f32 = 35.0;
const STAGE_ELEVATION: f32 = 28.0;
const MOTION_TURN: f32 = 25.0;
const MOTION_ELEVATION: f32 = 8.0;
const STANDING_ARM_DEGREES: f32 = -72.0;
const EXTENT_SAMPLES: u32 = 24;
const KEPT_FACTS: usize = 8;
const KEPT_ACTS: usize = 24;
const VMD_EXTENSION: &str = "vmd";

struct Mannequin {
    model: Arc<Model>,
    body: Arc<Body>,
    faces: Arc<Faces>,
    facts: Arc<Facts>,
    standing: HumanPose,
}

static MANNEQUIN: LazyLock<Mannequin> = LazyLock::new(|| {
    let model = animate_mannequin::model();
    let anatomy = Anatomy::recognize(&model);
    let body = Body::from_anatomy(&model, &anatomy).expect("the mannequin maps to a body");
    let mut standing = *body.rest_pose();
    for bone in [HumanBone::LeftUpperArm, HumanBone::RightUpperArm] {
        if let Some(muscle) = muscle_of(bone, Dof::Bend) {
            standing.muscles[muscle] = muscles()[muscle].value(STANDING_ARM_DEGREES.to_radians());
        }
    }
    for side in Side::BOTH {
        HandPreset::Relaxed.apply(side, &mut standing);
    }
    Mannequin {
        faces: Arc::new(Faces::from_model(&model)),
        facts: Arc::new(Facts::new(&model)),
        body: Arc::new(body),
        model,
        standing,
    }
});

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stand {
    Model,
    Stage,
    Accessory,
}

impl Stand {
    fn aim(self, facts: &Facts) -> Aim {
        match self {
            Self::Model if facts.humanoid => Aim::facing(
                facts.forward,
                PORTRAIT_TURN,
                PORTRAIT_ELEVATION,
                Fit::Silhouette,
            ),
            Self::Model | Self::Accessory => {
                Aim::facing(Vec3::Z, OBJECT_TURN, OBJECT_ELEVATION, Fit::Enclosing)
            }
            Self::Stage => Aim::facing(Vec3::Z, STAGE_TURN, STAGE_ELEVATION, Fit::Enclosing),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Format {
    Rikka,
    Vmd,
    Scene,
}

impl Format {
    fn of(path: &Path) -> Self {
        let named = |extension: &str| {
            path.extension()
                .is_some_and(|known| known.eq_ignore_ascii_case(extension))
        };
        if named(Motion::EXTENSION) {
            Self::Rikka
        } else if named(VMD_EXTENSION) {
            Self::Vmd
        } else {
            Self::Scene
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum ActKey {
    Still(Stamp, Stand),
    Motion(Stamp, ModelKey),
    Camera(Stamp),
}

struct Cast {
    key: ModelKey,
    model: Arc<Model>,
    body: Option<Arc<Body>>,
    faces: Arc<Faces>,
    facts: Arc<Facts>,
}

#[derive(Debug)]
pub(crate) struct Loader {
    facts: Memo<ModelKey, Arc<Facts>>,
    acts: Memo<ActKey, Arc<Setup>>,
}

impl Loader {
    pub(crate) fn new() -> Self {
        Self {
            facts: Memo::new(KEPT_FACTS),
            acts: Memo::new(KEPT_ACTS),
        }
    }

    pub(crate) fn setup(&self, sitter: &Sitter) -> Result<Arc<Setup>, BoothError> {
        match sitter {
            Sitter::Model(path) => self.still(path, Stand::Model),
            Sitter::Stage(path) => self.still(path, Stand::Stage),
            Sitter::Accessory(path) => self.still(path, Stand::Accessory),
            Sitter::Motion { motion, performer } => {
                let stamp = Stamp::of(motion)?;
                let cast = self.cast(performer);
                self.acts
                    .get_or_try(ActKey::Motion(stamp.clone(), cast.key.clone()), || {
                        motion_setup(&stamp, cast).map(Arc::new)
                    })
            }
            Sitter::Camera(path) => {
                let stamp = Stamp::of(path)?;
                self.acts.get_or_try(ActKey::Camera(stamp.clone()), || {
                    camera_setup(&stamp).map(Arc::new)
                })
            }
        }
    }

    fn still(&self, path: &Path, stand: Stand) -> Result<Arc<Setup>, BoothError> {
        let stamp = Stamp::of(path)?;
        let act = ActKey::Still(stamp.clone(), stand);
        if let Some(setup) = self.acts.get(&act) {
            return Ok(setup);
        }
        let model = Arc::new(Import::open(path, registry()).run()?);
        let facts = Arc::new(Facts::new(&model));
        let performance = rested(&facts);
        let view = frame(&model, &performance, stand.aim(&facts), 0.0, 0.0);
        let key = ModelKey::File(stamp.clone());
        let kept = Setup {
            key: key.clone(),
            source: Source::File(stamp),
            performance,
            view: view.clone(),
            still: 0.0,
            length: 0.0,
        };
        self.acts.insert(act, Arc::new(kept));
        Ok(Arc::new(Setup {
            key,
            source: Source::Model(model),
            performance: rested(&facts),
            view,
            still: 0.0,
            length: 0.0,
        }))
    }

    fn cast(&self, performer: &Performer) -> Cast {
        match performer {
            Performer::Figure {
                key,
                model,
                body,
                faces,
            } => {
                let key = ModelKey::Figure(*key);
                let facts = match self.facts.get(&key) {
                    Some(facts) => facts,
                    None => {
                        let facts = Arc::new(Facts::new(model));
                        self.facts.insert(key.clone(), Arc::clone(&facts));
                        facts
                    }
                };
                Cast {
                    key,
                    model: Arc::clone(model),
                    body: body.clone(),
                    faces: Arc::clone(faces),
                    facts,
                }
            }
            Performer::Mannequin => {
                let mannequin = &*MANNEQUIN;
                Cast {
                    key: ModelKey::Mannequin,
                    model: Arc::clone(&mannequin.model),
                    body: Some(Arc::clone(&mannequin.body)),
                    faces: Arc::clone(&mannequin.faces),
                    facts: Arc::clone(&mannequin.facts),
                }
            }
        }
    }
}

fn seconds(frame: u32) -> f32 {
    frame as f32 / FRAMES_PER_SECOND
}

fn rested(facts: &Arc<Facts>) -> Performance {
    Performance {
        posture: Posture::Rest,
        body: None,
        facts: Arc::clone(facts),
    }
}

fn motion_setup(stamp: &Stamp, cast: Cast) -> Result<Setup, BoothError> {
    let recipient = Recipient {
        model: Arc::clone(&cast.model),
        body: cast.body.clone(),
        faces: Arc::clone(&cast.faces),
    };
    let mut retarget = retarget(stamp, recipient)?;
    let last = retarget.frames().saturating_sub(1);
    let eye = Shot::default().eye();
    let still = match &cast.body {
        Some(body) => expressive_frame(last, body.rest_pose(), |frame| {
            retarget.pose_at(frame as f32, eye)
        }),
        None => last / 2,
    };
    let aim = Aim::facing(
        cast.facts.forward,
        MOTION_TURN,
        MOTION_ELEVATION,
        Fit::Silhouette,
    );
    let performance = Performance {
        posture: Posture::Retarget(Box::new(Mutex::new(retarget))),
        body: cast.body,
        facts: cast.facts,
    };
    let view = frame(
        &cast.model,
        &performance,
        aim,
        seconds(still),
        seconds(last),
    );
    Ok(Setup {
        key: cast.key,
        source: Source::Model(cast.model),
        performance,
        view,
        still: seconds(still),
        length: seconds(last),
    })
}

fn frame(model: &Arc<Model>, performance: &Performance, aim: Aim, still: f32, length: f32) -> View {
    let mut instance = Instance::new(Arc::clone(model));
    let still_bounds = performance.extent(&mut instance, [still]);
    let samples = (0..=EXTENT_SAMPLES).map(|sample| length * sample as f32 / EXTENT_SAMPLES as f32);
    let extent = match length > 0.0 {
        true => performance
            .extent(&mut instance, samples)
            .union(still_bounds),
        false => still_bounds,
    };
    View::Framed {
        still: still_bounds,
        extent,
        aim,
    }
}

fn camera_setup(stamp: &Stamp) -> Result<Setup, BoothError> {
    let motion = read_camera(stamp)?;
    let mannequin = &*MANNEQUIN;
    Ok(Setup {
        key: ModelKey::Mannequin,
        source: Source::Model(Arc::clone(&mannequin.model)),
        performance: Performance {
            posture: Posture::Standing(Box::new(mannequin.standing)),
            body: Some(Arc::clone(&mannequin.body)),
            facts: Arc::clone(&mannequin.facts),
        },
        still: seconds(camera_frame(&motion)),
        length: seconds(motion.last_frame()),
        view: View::Camera(Arc::new(motion)),
    })
}

fn read_vmd(stamp: &Stamp) -> Result<Vmd, BoothError> {
    let options = ImportOptions::default();
    options.limits.check(Limit::FileBytes, stamp.size())?;
    let bytes = std::fs::read(stamp.path()).map_err(BoothError::Read)?;
    Ok(Vmd::read(&bytes, &options)?)
}

fn retarget(stamp: &Stamp, recipient: Recipient) -> Result<Retarget, BoothError> {
    let path = stamp.path();
    match Format::of(path) {
        Format::Rikka => match Motion::load(path)? {
            Motion::Figure(take) if take.human.is_some() && recipient.body.is_none() => {
                Err(BoothError::NeedsBody)
            }
            Motion::Figure(take) => Ok(Retarget::from_take(recipient, Arc::from(take))),
            _ => Err(BoothError::NoMotion),
        },
        Format::Vmd => {
            let vmd = read_vmd(stamp)?;
            match vmd.bones.is_empty() && vmd.morphs.is_empty() && vmd.properties.is_empty() {
                true => Err(BoothError::NoMotion),
                false => Ok(Retarget::from_vmd(recipient, &vmd)),
            }
        }
        Format::Scene => {
            let model = Import::open(path, registry()).run()?;
            let clip = Clip::new(Arc::new(model), 0).ok_or(BoothError::NoMotion)?;
            Ok(Retarget::from_clip(recipient, &clip))
        }
    }
}

fn read_camera(stamp: &Stamp) -> Result<CameraMotion, BoothError> {
    let motion = match Format::of(stamp.path()) {
        Format::Rikka => match Motion::load(stamp.path())? {
            Motion::Camera(motion) => *motion,
            _ => return Err(BoothError::NoCamera),
        },
        Format::Vmd => read_vmd(stamp)?.cameras.into_iter().collect(),
        Format::Scene => return Err(BoothError::NoCamera),
    };
    match motion.is_empty() {
        true => Err(BoothError::NoCamera),
        false => Ok(motion),
    }
}
