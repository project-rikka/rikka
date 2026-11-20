use std::fmt;

use animate_body::{Dof, EYES, Expression, HandPreset, HumanBone, Side, muscles, muscles_of};
use animate_cloth::{Knob, preset_id};
use animate_edit::{CurvePreset, EasingPreset};
use animate_motion::{Part, Tag};
use animate_wind::WindSetting;
use rikka_keyframe::{Ease, Family};
use rikka_mmd::VmdCurve;
use rikka_physics_tailor::Preset;

pub(crate) const DOFS: [Dof; 3] = [Dof::Bend, Dof::Side, Dof::Twist];

#[must_use]
pub fn dof_name(dof: Dof) -> &'static str {
    match dof {
        Dof::Bend => "bend",
        Dof::Side => "side",
        Dof::Twist => "twist",
    }
}

pub(crate) fn dof_named(name: &str) -> Option<Dof> {
    DOFS.into_iter().find(|dof| dof_name(*dof) == name)
}

#[must_use]
pub fn side_name(side: Side) -> &'static str {
    match side {
        Side::Left => "left",
        Side::Right => "right",
    }
}

#[must_use]
pub fn is_posable(index: usize) -> bool {
    !EYES.contains(&index)
}

pub(crate) fn joints() -> Vec<HumanBone> {
    let mut joints: Vec<HumanBone> = Vec::new();
    for (index, muscle) in muscles().iter().enumerate() {
        if is_posable(index) && !joints.contains(&muscle.bone) {
            joints.push(muscle.bone);
        }
    }
    joints
}

pub(crate) fn joint_named(name: &str) -> Option<HumanBone> {
    HumanBone::from_name(name).filter(|bone| muscles_of(*bone).any(is_posable))
}

#[must_use]
pub fn curve_name(curve: VmdCurve) -> Option<String> {
    EasingPreset::all()
        .iter()
        .find(|preset| preset.curve == curve)
        .map(|preset| preset_name(preset.preset))
}

pub(crate) fn curve_named(name: &str) -> Option<VmdCurve> {
    EasingPreset::all()
        .iter()
        .find(|preset| preset_name(preset.preset) == name)
        .map(|preset| preset.curve)
}

fn preset_name(preset: CurvePreset) -> String {
    let (lead, ease) = match preset {
        CurvePreset::Linear => return "linear".to_owned(),
        CurvePreset::Smooth => return "smooth".to_owned(),
        CurvePreset::Basic(ease) => ("ease", ease),
        CurvePreset::Penner(easing) => (family_name(easing.family), easing.ease),
    };
    let ending = match ease {
        Ease::In => "In",
        Ease::Out => "Out",
        Ease::InOut => "InOut",
    };
    format!("{lead}{ending}")
}

fn family_name(family: Family) -> &'static str {
    match family {
        Family::Linear => "linear",
        Family::Sine => "sine",
        Family::Quad => "quad",
        Family::Cubic => "cubic",
        Family::Quart => "quart",
        Family::Quint => "quint",
        Family::Expo => "expo",
        Family::Circ => "circ",
        Family::Back => "back",
        Family::Elastic => "elastic",
        Family::Bounce => "bounce",
    }
}

fn listed<'a>(names: impl IntoIterator<Item = &'a str>) -> String {
    names
        .into_iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<String>>()
        .join(", ")
}

pub(crate) fn curves() -> String {
    let names: Vec<String> = EasingPreset::all()
        .iter()
        .map(|preset| preset_name(preset.preset))
        .collect();
    listed(names.iter().map(String::as_str))
}

pub(crate) fn tags() -> String {
    listed(Tag::ALL.iter().map(|tag| tag.name()))
}

pub(crate) fn parts() -> String {
    listed(Part::ALL.iter().map(|part| part.name()))
}

pub(crate) fn hands() -> String {
    listed(HandPreset::ALL.iter().map(|hand| hand.name()))
}

pub(crate) fn expressions() -> String {
    listed(Expression::ALL.iter().map(|expression| expression.name()))
}

pub(crate) fn presets() -> String {
    listed(Preset::ALL.into_iter().map(preset_id))
}

pub(crate) fn knobs() -> String {
    listed(Knob::ALL.into_iter().map(Knob::id))
}

pub(crate) fn wind_settings() -> String {
    listed(WindSetting::ALL.iter().map(|setting| setting.id()))
}

pub(crate) fn wind_ranges() -> String {
    WindSetting::ALL
        .iter()
        .map(|setting| {
            let range = setting.range();
            format!(
                "`{}` {} to {}",
                setting.id(),
                number(*range.start()),
                number(*range.end())
            )
        })
        .collect::<Vec<String>>()
        .join(", ")
}

pub(crate) struct JointTable;

impl fmt::Display for JointTable {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, bone) in joints().into_iter().enumerate() {
            let ranges: Vec<String> = muscles()
                .iter()
                .filter(|muscle| muscle.bone == bone)
                .map(|muscle| {
                    let (lowest, highest) = muscle.angles(true);
                    format!(
                        "{} {}…{}",
                        dof_name(muscle.dof),
                        lowest.to_degrees().round(),
                        highest.to_degrees().round()
                    )
                })
                .collect();
            if index > 0 {
                writeln!(out)?;
            }
            write!(out, "  - `{}`: {}", bone.name(), ranges.join(", "))?;
        }
        Ok(())
    }
}

#[must_use]
pub fn number(value: f32) -> String {
    ((value * 10_000.0).round() / 10_000.0 + 0.0).to_string()
}
