use animate_body::{Expression, Panel};
use animate_cast::Tuning;
use animate_cloth::{Brush, Field, Knob};
use animate_fluid::{FluidSettings, Property};
use animate_hold::HoldId;
use animate_motion::Tag;
use rikka_physics_tailor::PieceKey;

use crate::card::Card;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    #[default]
    Motion,
    Node,
    Physics,
}

impl Pane {
    pub const ALL: [Pane; 3] = [Self::Motion, Self::Node, Self::Physics];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dial {
    Figure(Knob),
    Piece(Knob),
    Field(Field),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldDial {
    Reach,
    Ease,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Controls {
    pub tag: Tag,
    pub card: Option<Card>,
    pub split_eyes: bool,
    pub morph_panel: Panel,
    pub mood: Option<Expression>,
    pub automation_draft: Option<(Tuning, f32)>,
    pub paths: bool,
    pub linked: bool,
    pub pane: Pane,
    pub blob_draft: Option<f32>,
    pub liquid_draft: Option<(Property, animate_fluid::EmitterValue)>,
    pub scene_draft: Option<FluidSettings>,
    pub brush: Brush,
    pub painting: bool,
    pub piece: Option<PieceKey>,
    pub cloth_draft: Option<(Dial, f32)>,
    pub hold_draft: Option<(HoldId, HoldDial, f32)>,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            tag: Tag::default(),
            card: None,
            split_eyes: false,
            morph_panel: Panel::Eyebrow,
            mood: None,
            automation_draft: None,
            paths: true,
            linked: false,
            pane: Pane::default(),
            blob_draft: None,
            liquid_draft: None,
            scene_draft: None,
            brush: Brush::default(),
            painting: false,
            piece: None,
            cloth_draft: None,
            hold_draft: None,
        }
    }
}
