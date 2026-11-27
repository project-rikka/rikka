mod hit;
mod hold;
mod paint;
mod pour;
mod widget;

use std::borrow::Cow;

use animate_body::Side;
use animate_hold::HoldLane;
use animate_sheet::{Group, GroupKind, Lane, Track};
use rikka_app::prelude::*;

pub use widget::Dope;

use crate::wording;

#[derive(Clone, Debug, PartialEq)]
pub struct Names {
    pub switches: Label,
    pub unlisted: Label,
    pub unmatched: Label,
    pub camera: Label,
    pub parts: Label,
    pub morphs: [Label; 4],
    pub part: Vec<Label>,
    pub sections: Vec<Label>,
    pub settings: Vec<Label>,
    pub pour: Label,
    pub wind: Label,
    pub winds: Vec<Label>,
    pub placement: Label,
    pub props: Vec<Label>,
    pub anchor: Label,
    pub hands: [Label; 2],
    pub sound: Option<String>,
}

impl Names {
    fn hold_label(&self, lane: HoldLane) -> Label {
        match lane {
            HoldLane::Hand(Side::Left) => self.hands[0].clone(),
            HoldLane::Hand(Side::Right) => self.hands[1].clone(),
            HoldLane::Pin(at) => wording::pin_lane(at),
        }
    }
    fn group_name<'a>(&'a self, group: &'a Group) -> Cow<'a, str> {
        Cow::Borrowed(match &group.kind {
            GroupKind::Switches => self.switches.as_str(),
            GroupKind::Listed(name) => name,
            GroupKind::Region(part) => self.part.get(part.index()).map_or("", Label::as_str),
            GroupKind::Unlisted => self.unlisted.as_str(),
            GroupKind::Unmatched => self.unmatched.as_str(),
            GroupKind::Camera => self.camera.as_str(),
            GroupKind::Parts => self.parts.as_str(),
            GroupKind::Morphs(panel) => self.morphs[*panel as usize].as_str(),
            GroupKind::Look { group, title } => {
                return Cow::Owned(wording::look_group(group, title.as_deref()).to_string());
            }
            GroupKind::Fluid(section) => self
                .sections
                .get(*section as usize)
                .map_or("", Label::as_str),
            GroupKind::Wind => self.wind.as_str(),
            GroupKind::Prop => self.placement.as_str(),
        })
    }

    fn track_name<'a>(&'a self, track: &'a Track) -> &'a str {
        let label = |labels: &'a [Label], at: usize| labels.get(at).map_or("", Label::as_str);
        match &track.lane {
            Lane::Part(part) => label(&self.part, part.index()),
            Lane::Fluid(setting) => label(&self.settings, *setting as usize),
            Lane::Wind(setting) => label(&self.winds, *setting as usize),
            Lane::Prop(setting) => label(&self.props, *setting as usize),
            Lane::Anchor => self.anchor.as_str(),
            _ => track.name(),
        }
    }
}
