use std::sync::Arc;

use animate_body::Expression;
use animate_cast::{FigureId, PropRole};
use animate_fluid::{Anchor, EmitterId, EmitterSetting, EmitterValue};
use animate_hold::{Hold, HoldId};
use animate_motion::PoseSample;
use animate_prop::PropSetting;
use rikka_model::glam::Vec3;
use rikka_physics_tailor::ClothSettings;
use rikka_shader_graph::NodeId;

use crate::Marker;

#[derive(Clone, Debug, PartialEq)]
pub enum Gesture {
    Focus(Option<Marker>),
    Pose(Box<PoseSample>),
    Express(Expression, f32),
    Grasp(Box<Hold>),
    Hold(FigureId, Box<Hold>),
    ReleaseHold(FigureId, HoldId),
    Pour(EmitterId, EmitterSetting, EmitterValue),
    Attach {
        setting: EmitterSetting,
        anchor: Anchor,
        offset: Vec3,
    },
    Wind(animate_wind::WindSetting, f32),
    Tailor(FigureId, Arc<ClothSettings>),
    Place(NodeId, Vec3),
    Arrange {
        role: PropRole,
        settings: Vec<(PropSetting, f32)>,
    },
}
