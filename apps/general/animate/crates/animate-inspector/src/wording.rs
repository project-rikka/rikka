use animate_body::{Expression, HandPreset, Panel, Side};
use animate_cast::Tuning;
use animate_cloth::{Field, Knob, Tool, preset_id};
use animate_edit::Channel;
use animate_fluid::{Aim, Preset, Property, Shape};
use animate_motion::{GazeMode, Tag};
use rikka_i18n::{Label, tr};
use rikka_physics_tailor::Quality;

use crate::card::FaceTab;
use crate::controls::Pane;

pub(crate) fn litres(litres: f32) -> String {
    match litres < 0.1 {
        true => format!("{litres:.3}"),
        false => format!("{litres:.2}"),
    }
}

pub(crate) fn channel(channel: Channel) -> Label {
    match channel {
        Channel::X => tr!("animate.channel.x"),
        Channel::Y => tr!("animate.channel.y"),
        Channel::Z => tr!("animate.channel.z"),
        Channel::Rotation => tr!("animate.channel.rotation"),
        Channel::Distance => tr!("animate.channel.distance"),
        Channel::ViewAngle => tr!("animate.channel.view_angle"),
        Channel::Value => tr!("animate.channel.value"),
    }
}

pub(crate) fn tag(tag: Tag) -> Label {
    match tag {
        Tag::Smooth => tr!("animate.tag.smooth"),
        Tag::Ease => tr!("animate.tag.ease"),
        Tag::Hit => tr!("animate.tag.hit"),
        Tag::Linear => tr!("animate.tag.linear"),
        Tag::Step => tr!("animate.tag.step"),
        Tag::Settle => tr!("animate.tag.settle"),
    }
}

pub(crate) fn tuning(tuning: Tuning) -> Label {
    match tuning {
        Tuning::Secondary => tr!("animate.easy.secondary"),
        Tuning::Size => tr!("animate.style.size"),
        Tuning::Snap => tr!("animate.style.snap"),
        Tuning::Jiggle => tr!("animate.style.jiggle"),
        Tuning::Torso => tr!("animate.style.torso"),
        Tuning::Head => tr!("animate.style.head"),
        Tuning::Arms => tr!("animate.style.arms"),
        Tuning::Legs => tr!("animate.style.legs"),
        Tuning::Bounce => tr!("animate.style.bounce"),
    }
}

pub(crate) fn hand(preset: HandPreset) -> Label {
    match preset {
        HandPreset::Natural => tr!("animate.hand.natural"),
        HandPreset::Open => tr!("animate.hand.open"),
        HandPreset::Fist => tr!("animate.hand.fist"),
        HandPreset::Peace => tr!("animate.hand.peace"),
        HandPreset::Point => tr!("animate.hand.point"),
        HandPreset::ThumbsUp => tr!("animate.hand.thumbs_up"),
        HandPreset::Ok => tr!("animate.hand.ok"),
        HandPreset::Rock => tr!("animate.hand.rock"),
        HandPreset::Gun => tr!("animate.hand.gun"),
        HandPreset::Relaxed => tr!("animate.hand.relaxed"),
        HandPreset::Pinch => tr!("animate.hand.pinch"),
    }
}

pub(crate) fn eye(side: Side) -> Label {
    match side {
        Side::Left => tr!("animate.gaze.left_eye"),
        Side::Right => tr!("animate.gaze.right_eye"),
    }
}

pub(crate) fn panel(panel: Panel) -> Label {
    match panel {
        Panel::Eyebrow => tr!("animate.morph.eyebrow"),
        Panel::Eye => tr!("animate.morph.eye"),
        Panel::Mouth => tr!("animate.morph.mouth"),
        Panel::Other => tr!("animate.morph.other"),
    }
}

pub(crate) fn face_tab(tab: FaceTab) -> Label {
    match tab {
        FaceTab::Eyes => tr!("animate.card.eyes"),
        FaceTab::Expression => tr!("animate.card.expression"),
        FaceTab::Mouth => tr!("animate.card.mouth"),
    }
}

pub(crate) fn expression(expression: Expression) -> Label {
    match expression {
        Expression::Happy => tr!("animate.face.happy"),
        Expression::Angry => tr!("animate.face.angry"),
        Expression::Sad => tr!("animate.face.sad"),
        Expression::Relaxed => tr!("animate.face.relaxed"),
        Expression::Surprised => tr!("animate.face.surprised"),
        Expression::Blink => tr!("animate.face.blink"),
        Expression::BlinkLeft => tr!("animate.face.blink_left"),
        Expression::BlinkRight => tr!("animate.face.blink_right"),
        Expression::Aa => tr!("animate.face.aa"),
        Expression::Ih => tr!("animate.face.ih"),
        Expression::Ou => tr!("animate.face.ou"),
        Expression::Ee => tr!("animate.face.ee"),
        Expression::Oh => tr!("animate.face.oh"),
    }
}

pub(crate) fn gaze(mode: GazeMode) -> Label {
    match mode {
        GazeMode::Off => tr!("animate.gaze.off"),
        GazeMode::Point => tr!("animate.gaze.point"),
        GazeMode::Camera => tr!("animate.gaze.camera"),
    }
}

pub(crate) fn pane(pane: Pane) -> Label {
    match pane {
        Pane::Motion => tr!("animate.inspector.motion"),
        Pane::Node => tr!("animate.inspector.node"),
        Pane::Physics => tr!("animate.inspector.physics"),
    }
}

pub(crate) fn shape(shape: &Shape) -> Label {
    tr!(&format!("animate.liquid.shapes.{}", shape.id()))
}

pub(crate) fn aim(aim: Aim) -> Label {
    tr!(&format!("animate.liquid.aims.{}", aim.id()))
}

pub(crate) fn preset(preset: Preset) -> Label {
    tr!(&format!("animate.liquid.preset.{}", preset.id()))
}

pub(crate) fn property(property: Property) -> Label {
    tr!(&format!("animate.liquid.property.{}", property.id()))
}

pub(crate) fn brush(tool: Tool) -> Label {
    tr!(&format!("animate.cloth.tool.{}", tool.id()))
}

pub(crate) fn knob(knob: Knob) -> Label {
    tr!(&format!("animate.cloth.knob.{}", knob.id()))
}

pub(crate) fn knob_ends(knob: Knob) -> (Label, Label) {
    (
        tr!(&format!("animate.cloth.knob.{}_least", knob.id())),
        tr!(&format!("animate.cloth.knob.{}_most", knob.id())),
    )
}

pub(crate) fn style_field(field: Field) -> Label {
    tr!(&format!("animate.cloth.field.{}", field.id()))
}

pub(crate) fn style_section(section: animate_cloth::FieldSection) -> Label {
    tr!(&format!("animate.cloth.section.{}", section.id()))
}

pub(crate) fn garment(preset: rikka_physics_tailor::Preset) -> Label {
    tr!(&format!("animate.cloth.preset.{}", preset_id(preset)))
}

pub(crate) fn quality(quality: Quality) -> Label {
    match quality {
        Quality::Low => tr!("animate.cloth.quality.low"),
        Quality::Medium => tr!("animate.cloth.quality.medium"),
        Quality::High => tr!("animate.cloth.quality.high"),
    }
}
