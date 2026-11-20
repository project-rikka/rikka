use crate::look::{GraphEdits, LookKeys, LookQuery, LookReset, MaterialEdits, NodesQuery};
use animate_body::{Dof, Expression, HandPreset, HumanBone, Limb, Side};
use animate_cloth::Knob;
use animate_motion::{GazeMode, Part, Tag};
use animate_sheet::Lane;
use animate_solve::EyeDirection;
use animate_wind::WindSetting;
use rikka_mmd::VmdCurve;
use rikka_model_core::glam::Vec3;
use rikka_physics_tailor::Preset;

#[derive(Clone, Debug, PartialEq)]
pub enum ToolRequest {
    Scene,
    Figure {
        figure: String,
        frame: Option<u32>,
    },
    Pose {
        figure: String,
        keys: Vec<PoseKey>,
    },
    Look(LookQuery),
    Nodes(NodesQuery),
    EditGraph(GraphEdits),
    Materials(MaterialEdits),
    LookKeys(LookKeys),
    ResetLook(LookReset),
    Camera {
        keys: Vec<CameraKey>,
    },
    Keys(KeysRequest),
    Timeline {
        frame: Option<u32>,
        play: Option<bool>,
    },
    Render(RenderRequest),
    Cloth {
        figure: String,
    },
    SetCloth {
        figure: String,
        feel: Vec<(Knob, f32)>,
        pieces: Vec<PieceRequest>,
    },
    Wind {
        frame: Option<u32>,
    },
    SetWind(WindEdit),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PieceRequest {
    pub piece: String,
    pub preset: Option<Preset>,
    pub enabled: Option<bool>,
    pub feel: Vec<(Knob, f32)>,
    pub reset: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindEdit {
    pub resting: Vec<(WindSetting, f32)>,
    pub keys: Vec<WindKeyRequest>,
    pub clear: Vec<WindSetting>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindKeyRequest {
    pub frame: u32,
    pub values: Vec<(WindSetting, f32)>,
    pub curve: Option<VmdCurve>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PoseKey {
    pub frame: u32,
    pub tag: Option<Tag>,
    pub rest: Vec<Part>,
    pub joints: Vec<JointRotation>,
    pub hips: Option<Hips>,
    pub hands: Vec<(Side, HandPreset)>,
    pub expressions: Vec<(Expression, f32)>,
    pub morphs: Vec<(String, f32)>,
    pub gaze: Option<GazeRequest>,
    pub reach: Vec<(Extremity, Vec3)>,
    pub pins: Vec<(Limb, bool)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointRotation {
    pub bone: HumanBone,
    pub dof: Dof,
    pub degrees: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hips {
    pub position: Option<Vec3>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    pub roll: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GazeRequest {
    pub mode: Option<GazeMode>,
    pub target: Option<Vec3>,
    pub weight: Option<f32>,
    pub head_follow: Option<f32>,
    pub left_eye: Option<EyeDirection>,
    pub right_eye: Option<EyeDirection>,
}

impl GazeRequest {
    #[must_use]
    pub fn eye(&self, side: Side) -> Option<EyeDirection> {
        match side {
            Side::Left => self.left_eye,
            Side::Right => self.right_eye,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Extremity {
    Hand(Side),
    Foot(Side),
}

impl Extremity {
    #[must_use]
    pub fn limb(self) -> Limb {
        match self {
            Self::Hand(Side::Left) => Limb::LeftArm,
            Self::Hand(Side::Right) => Limb::RightArm,
            Self::Foot(side) => side.leg(),
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        self.limb().chain()[2].name()
    }

    pub(crate) const ALL: [Self; 4] = [
        Self::Hand(Side::Left),
        Self::Hand(Side::Right),
        Self::Foot(Side::Left),
        Self::Foot(Side::Right),
    ];
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraKey {
    pub frame: u32,
    pub placement: Placement,
    pub curve: Option<VmdCurve>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub target: Vec3,
    pub stand: CameraPosition,
    pub roll: f32,
    pub fov: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CameraPosition {
    Eye(Vec3),
    Orbit {
        yaw: f32,
        elevation: f32,
        distance: f32,
    },
}

impl Placement {
    #[must_use]
    pub fn orbit(&self) -> (f32, f32, f32) {
        match self.stand {
            CameraPosition::Orbit {
                yaw,
                elevation,
                distance,
            } => (yaw, elevation, distance),
            CameraPosition::Eye(eye) => {
                let away = eye - self.target;
                let distance = away.length();
                if distance <= f32::EPSILON {
                    return (0.0, 0.0, 0.0);
                }
                let along = away / distance;
                (
                    along.x.atan2(along.z).to_degrees(),
                    along.y.clamp(-1.0, 1.0).asin().to_degrees(),
                    distance,
                )
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeysSubject {
    Figure(String),
    Camera,
    Look,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeysRequest {
    pub subject: KeysSubject,
    pub action: KeysAction,
    pub lanes: Vec<Lane>,
    pub from: Option<u32>,
    pub to: Option<u32>,
}

impl KeysRequest {
    #[must_use]
    pub fn covers(&self, lane: &Lane, frame: u32) -> bool {
        self.from.is_none_or(|from| frame >= from)
            && self.to.is_none_or(|to| frame <= to)
            && (self.lanes.is_empty() || self.lanes.contains(lane))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeysAction {
    List,
    Delete,
    Shift(i64),
    Curve(VmdCurve),
    Tag(Tag),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RenderView {
    Film,
    View,
    Placed(Placement),
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderRequest {
    pub frames: Vec<u32>,
    pub view: RenderView,
    pub width: u32,
}
