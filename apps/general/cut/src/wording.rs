use std::path::Path;

use cut_filmstrip::Mode;
use cut_project::{
    Action, AnimationPresetKind, Art, Attribute, EffectSlot, Error, Format, Graphic, Height,
    KeyFit, LoopKind, Mismatch, Order, Piece, Point, Redraw, Region, Smooth, Switch,
    TransitionAlignment, TransitionFamily, TransitionKind, TransitionKnob,
};
use cut_waveform::Scale;
use rikka_app::prelude::*;
use rikka_figure::ShapeKind;
use rikka_timeline::{ease_label, family_label};

use crate::command::Command;
use crate::cut::{EditDirection, PresetSlot};
use crate::probe::ProbeError;
use crate::quality::Quality;
use crate::relink::Refusal;
use crate::tool::Tool;
use crate::transport::Direction;
use crate::trimmer::Trimmer;

pub(crate) fn layer(index: usize) -> Label {
    tr!("cut.layer", number = &format!("{index:02}"))
}

pub(crate) fn offline() -> Label {
    tr!("cut.monitor.offline")
}

pub(crate) fn format(format: Format) -> String {
    tr!(
        "cut.format",
        width = &format.width.to_string(),
        height = &format.height.to_string(),
        rate = &format.rate.to_string()
    )
    .to_string()
}

pub(crate) fn command(command: Command) -> Label {
    match command {
        Command::New => tr!("cut.command.new"),
        Command::Open => tr!("cut.command.open"),
        Command::Import => tr!("cut.command.import"),
        Command::Save => tr!("cut.command.save"),
        Command::SaveAs => tr!("cut.command.save_as"),
        Command::Export => tr!("cut.command.export"),
        Command::ExportFrame => tr!("cut.command.export_frame"),
        Command::Quit => tr!("cut.command.quit"),
        Command::Undo => tr!("cut.command.undo"),
        Command::Redo => tr!("cut.command.redo"),
        Command::Cut => tr!("cut.command.cut"),
        Command::Copy => tr!("cut.command.copy"),
        Command::Paste => tr!("cut.command.paste"),
        Command::PasteAttributes => tr!("cut.command.paste_attributes"),
        Command::PasteEffects => tr!("cut.command.paste_effects"),
        Command::RemoveEffects => tr!("cut.command.remove_effects"),
        Command::RelinkMedia => tr!("cut.command.relink_media"),
        Command::ReplaceMedia => tr!("cut.command.replace_media"),
        Command::SelectAll => tr!("cut.command.select_all"),
        Command::Deselect => tr!("cut.command.deselect"),
        Command::Delete => tr!("cut.command.delete"),
        Command::RippleDelete => tr!("cut.command.ripple_delete"),
        Command::Split => tr!("cut.command.split"),
        Command::Snapping => tr!("cut.command.snapping"),
        Command::PlayPause => tr!("cut.command.play_pause"),
        Command::Stop => tr!("cut.command.stop"),
        Command::Pause => tr!("cut.command.pause"),
        Command::Shuttle(Direction::Forward) => tr!("cut.command.shuttle_forward"),
        Command::Shuttle(Direction::Backward) => tr!("cut.command.shuttle_backward"),
        Command::Step(frames) if frames < 0 => tr!("cut.command.step_back"),
        Command::Step(_) => tr!("cut.command.step_forward"),
        Command::PreviousEdit => tr!("cut.command.previous_edit"),
        Command::NextEdit => tr!("cut.command.next_edit"),
        Command::GoToStart => tr!("cut.command.go_to_start"),
        Command::GoToEnd => tr!("cut.command.go_to_end"),
        Command::MarkIn => tr!("cut.command.mark_in"),
        Command::MarkOut => tr!("cut.command.mark_out"),
        Command::ClearMarks => tr!("cut.command.clear_marks"),
        Command::GoToIn => tr!("cut.command.go_to_in"),
        Command::GoToOut => tr!("cut.command.go_to_out"),
        Command::ZoomIn => tr!("cut.command.zoom_in"),
        Command::ZoomOut => tr!("cut.command.zoom_out"),
        Command::ZoomToFit => tr!("cut.command.zoom_to_fit"),
        Command::Arm(tool) => tool_name(tool),
        Command::Add(tool) => tool_name(tool),
        Command::AddRange => tr!("cut.command.add_range"),
        Command::AddWhole => tr!("cut.command.add_whole"),
        Command::AddCamera => tr!("cut.camera.add"),
        Command::AddEffect(kind) => kind.name(),
        Command::PreviousKey => tr!("cut.command.previous_key"),
        Command::NextKey => tr!("cut.command.next_key"),
        Command::KeyLanes => tr!("cut.keys.lanes"),
        Command::GraphEditor => tr!("cut.keys.graph"),
        Command::Interpolate(interpolation) => self::interpolation(interpolation),
        Command::Quality(selected) => quality(selected),
        Command::Thumbnails(mode) => thumbnails(mode),
        Command::Waveform(scale) => waveform(scale),
        Command::LayerHeight(height) => layer_height(height),
        Command::Trimmer(trimmer) => self::trimmer(trimmer),
        Command::RippleTrimTo(EditDirection::Previous) => tr!("cut.command.ripple_trim_previous"),
        Command::RippleTrimTo(EditDirection::Next) => tr!("cut.command.ripple_trim_next"),
        Command::TrimTo(EditDirection::Previous) => tr!("cut.command.trim_previous"),
        Command::TrimTo(EditDirection::Next) => tr!("cut.command.trim_next"),
        Command::ExtendEdit => tr!("cut.command.extend_edit"),
        Command::AddMarker => tr!("cut.command.add_marker"),
        Command::AddClipMarker => tr!("cut.command.add_clip_marker"),
        Command::PreviousMarker => tr!("cut.command.previous_marker"),
        Command::NextMarker => tr!("cut.command.next_marker"),
        Command::EditMarker => tr!("cut.command.edit_marker"),
        Command::DeleteMarker => tr!("cut.command.delete_marker"),
        Command::ClearMarkers => tr!("cut.command.clear_markers"),
        Command::ProjectSettings => tr!("cut.command.project_settings"),
    }
}

pub(crate) fn action(action: Action) -> Label {
    match action {
        Action::Place => tr!("cut.action.place"),
        Action::Insert => tr!("cut.action.insert"),
        Action::Redraw(Redraw::Move) => tr!("cut.action.redraw_move"),
        Action::Redraw(Redraw::Resize) => tr!("cut.action.redraw_resize"),
        Action::Redraw(Redraw::Rotate) => tr!("cut.action.redraw_rotate"),
        Action::Redraw(Redraw::Style) => tr!("cut.action.redraw_style"),
        Action::Redraw(Redraw::Text) => tr!("cut.action.redraw_text"),
        Action::Effects => tr!("cut.action.effects"),
        Action::Key => tr!("cut.action.key"),
        Action::AnimationPresets => tr!("cut.action.acts"),
        Action::Tween => tr!("cut.action.tween"),
        Action::Transitions => tr!("cut.action.transitions"),
        Action::Paste => tr!("cut.action.paste"),
        Action::Move => tr!("cut.action.move"),
        Action::Trim => tr!("cut.action.trim"),
        Action::Slip => tr!("cut.action.slip"),
        Action::RippleTrim => tr!("cut.action.ripple_trim"),
        Action::Roll => tr!("cut.action.roll"),
        Action::Slide => tr!("cut.action.slide"),
        Action::Extract => tr!("cut.action.extract"),
        Action::Split => tr!("cut.action.split"),
        Action::Delete => tr!("cut.action.delete"),
        Action::Cut => tr!("cut.action.cut"),
        Action::RippleDelete => tr!("cut.action.ripple_delete"),
        Action::Switch(Switch::Shown) => tr!("cut.action.layer_shown"),
        Action::Switch(Switch::Locked) => tr!("cut.action.layer_lock"),
        Action::Resize => tr!("cut.action.layer_height"),
        Action::Mark(Point::In) => tr!("cut.action.mark_in"),
        Action::Mark(Point::Out) => tr!("cut.action.mark_out"),
        Action::ClearMarks => tr!("cut.action.clear_marks"),
        Action::Marker => tr!("cut.action.marker"),
        Action::Reformat => tr!("cut.action.reformat"),
        Action::PasteAttributes => tr!("cut.action.paste_attributes"),
        Action::RemoveEffects => tr!("cut.action.remove_effects"),
        Action::Relink => tr!("cut.action.relink"),
    }
}

pub(crate) fn attribute(attribute: Attribute, from: &Piece) -> Label {
    match attribute {
        Attribute::Placement => tr!("cut.panel.drawing"),
        Attribute::Picture => tr!("cut.panel.picture"),
        Attribute::Speed => tr!("cut.panel.speed"),
        Attribute::AnimationPresets => tr!("cut.acts.group"),
        Attribute::Effect(slot) => from.effects().get(slot).map_or_else(
            || tr!("cut.panel.effects"),
            |slot| slot.effect.kind().name(),
        ),
        Attribute::Sound => tr!("cut.panel.sound"),
        Attribute::Camera => tr!("cut.camera.group"),
    }
}

pub(crate) fn key_fit(keys: KeyFit) -> Label {
    match keys {
        KeyFit::Maintain => tr!("cut.attributes.maintain"),
        KeyFit::Stretch => tr!("cut.attributes.stretch"),
    }
}

fn mismatch(mismatch: Mismatch) -> Label {
    match mismatch {
        Mismatch::MissingPicture => tr!("cut.relink.missing_picture"),
        Mismatch::UnexpectedPicture => tr!("cut.relink.unexpected_picture"),
        Mismatch::MissingSound => tr!("cut.relink.missing_sound"),
        Mismatch::TooShort { needed, frames } => {
            tr!("cut.relink.too_short", needed = needed, frames = frames)
        }
    }
}

pub(crate) fn relink_refusal(refusal: &Refusal) -> Label {
    match refusal {
        Refusal::Probe(error) => Label::from(probe_reason(error)),
        Refusal::Unfit(Error::Mismatch { reason, .. }) => mismatch(*reason),
        Refusal::Unfit(error) => refused(error),
        Refusal::Unlike => tr!("cut.relink.unlike"),
    }
}

pub(crate) fn searched(found: usize, unreadable: usize) -> Label {
    let found = match found {
        0 => tr!("cut.relink.none_found"),
        count => tr!("cut.relink.searched", count = count),
    };
    match unreadable {
        0 => found,
        count => tr!(
            "cut.relink.unreadable",
            found = &found.to_string(),
            count = count
        ),
    }
}

pub(crate) fn refused(error: &Error) -> Label {
    match error {
        Error::Locked => tr!("cut.refused.locked"),
        Error::OutOfReach => tr!("cut.refused.out_of_reach"),
        Error::TooShort(path) => tr!("cut.refused.too_short", name = &file_name(path)),
        Error::NothingToPaste => tr!("cut.refused.nothing_to_paste"),
        Error::NoEffects => tr!("cut.refused.no_effects"),
        Error::Mismatch { path, reason } => tr!(
            "cut.refused.mismatch",
            name = &file_name(path),
            reason = &mismatch(*reason).to_string()
        ),
        other => tr!("cut.refused.other", reason = &other.to_string()),
    }
}

fn probe_reason(error: &ProbeError) -> String {
    match error {
        ProbeError::StillImage => tr!("cut.import.still").to_string(),
        ProbeError::Unsupported => tr!("cut.import.unknown").to_string(),
        ProbeError::UnknownDuration => tr!("cut.import.endless").to_string(),
        ProbeError::UnknownFrameRate => tr!("cut.import.unpaced").to_string(),
        ProbeError::Video(error) => error.to_string(),
        ProbeError::Audio(error) => error.to_string(),
    }
}

pub(crate) fn unimported(path: &Path, error: &ProbeError, count: usize) -> Label {
    let reason = probe_reason(error);
    match count {
        1 => tr!(
            "cut.import.refused",
            name = &file_name(path),
            reason = &reason
        ),
        _ => tr!(
            "cut.import.refused_many",
            name = &file_name(path),
            reason = &reason,
            others = count - 1
        ),
    }
}

pub(crate) fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

pub(crate) fn trimmer(trimmer: Trimmer) -> Label {
    match trimmer {
        Trimmer::Select => tr!("cut.trimmer.select"),
        Trimmer::Ripple => tr!("cut.trimmer.ripple"),
        Trimmer::Roll => tr!("cut.trimmer.roll"),
        Trimmer::Slip => tr!("cut.trimmer.slip"),
        Trimmer::Slide => tr!("cut.trimmer.slide"),
    }
}

pub(crate) fn tool_name(tool: Tool) -> Label {
    match tool {
        Tool::Select => tr!("cut.tool.select"),
        Tool::Anchor => tr!("cut.stage.anchor"),
        Tool::Text => tr!("cut.tool.text"),
        Tool::Shape(kind) => shape(kind),
        Tool::Line => tr!("cut.tool.line"),
        Tool::Arrow => tr!("cut.tool.arrow"),
        Tool::Range(_) => tr!("cut.tool.range"),
    }
}

pub(crate) fn shape(kind: ShapeKind) -> Label {
    match kind {
        ShapeKind::Rectangle => tr!("cut.shape.rectangle"),
        ShapeKind::RoundedRectangle => tr!("cut.shape.rounded_rectangle"),
        ShapeKind::Ellipse => tr!("cut.shape.ellipse"),
        ShapeKind::Triangle => tr!("cut.shape.triangle"),
        ShapeKind::Polygon => tr!("cut.shape.polygon"),
        ShapeKind::Star => tr!("cut.shape.star"),
    }
}

pub(crate) fn graphic(graphic: &Graphic) -> String {
    match &graphic.art {
        Art::Text { lettering, .. } => lettering
            .text
            .lines()
            .find(|line| !line.trim().is_empty())
            .map_or_else(|| tr!("cut.tool.text").to_string(), str::to_owned),
        Art::Shape {
            shape: shape_art, ..
        } => shape(shape_art.kind()).to_string(),
        Art::Line { arrow: true, .. } => tr!("cut.tool.arrow").to_string(),
        Art::Line { arrow: false, .. } => tr!("cut.tool.line").to_string(),
    }
}

pub(crate) fn camera() -> Label {
    tr!("cut.art.camera")
}

pub(crate) fn area(shape: Option<ShapeKind>) -> Label {
    match shape {
        Some(kind) => self::shape(kind),
        None => tr!("cut.area.whole"),
    }
}

pub(crate) fn region(region: &Region, effects: &[EffectSlot]) -> String {
    let names: Vec<String> = effects
        .iter()
        .filter(|slot| slot.enabled)
        .map(|slot| slot.effect.kind().name().to_string())
        .collect();
    match (names.is_empty(), region.is_whole()) {
        (false, _) => names.join(" · "),
        (true, true) => tr!("cut.command.add_whole").to_string(),
        (true, false) => tr!("cut.tool.range").to_string(),
    }
}

pub(crate) fn quality(quality: Quality) -> Label {
    match quality {
        Quality::Auto => tr!("cut.quality.auto"),
        Quality::Full => tr!("cut.quality.full"),
        Quality::Half => tr!("cut.quality.half"),
        Quality::Quarter => tr!("cut.quality.quarter"),
    }
}

fn thumbnails(mode: Mode) -> Label {
    match mode {
        Mode::Continuous => tr!("cut.timeline.thumbnails_continuous"),
        Mode::Head => tr!("cut.timeline.thumbnails_head"),
        Mode::HeadAndTail => tr!("cut.timeline.thumbnails_ends"),
        Mode::Off => tr!("cut.timeline.thumbnails_off"),
    }
}

fn waveform(scale: Scale) -> Label {
    match scale {
        Scale::Logarithmic => tr!("cut.timeline.waveform_logarithmic"),
        Scale::Linear => tr!("cut.timeline.waveform_linear"),
    }
}

fn layer_height(height: Height) -> Label {
    match height {
        Height::SMALL => tr!("cut.timeline.height_small"),
        Height::MEDIUM => tr!("cut.timeline.height_medium"),
        Height::LARGE => tr!("cut.timeline.height_large"),
        other => tr!("cut.timeline.height_px", px = &other.px().to_string()),
    }
}

pub(crate) fn interpolation(interpolation: cut_project::Interpolation) -> Label {
    use cut_project::Interpolation;
    match interpolation {
        Interpolation::Linear => tr!("cut.keys.linear"),
        Interpolation::Bezier => tr!("cut.keys.bezier"),
        Interpolation::Continuous => tr!("cut.keys.continuous"),
        Interpolation::Auto => tr!("cut.keys.auto"),
        Interpolation::Hold => tr!("cut.keys.hold"),
        Interpolation::EasyEase => tr!("cut.keys.easy_ease"),
        Interpolation::EasyEaseIn => tr!("cut.keys.easy_ease_in"),
        Interpolation::EasyEaseOut => tr!("cut.keys.easy_ease_out"),
        Interpolation::Preset(easing) => tr!(
            "cut.keys.preset",
            curve = &family_label(easing.family).to_string(),
            ease = &ease_label(easing.ease).to_string()
        ),
    }
}

pub(crate) fn placement_field(field: cut_project::PlacementField) -> Label {
    use cut_project::PlacementField;
    match field {
        PlacementField::Position => tr!("cut.rows.position"),
        PlacementField::Depth => tr!("cut.rows.depth"),
        PlacementField::Anchor => tr!("cut.rows.anchor"),
        PlacementField::Zoom => tr!("cut.panel.zoom"),
        PlacementField::Aspect => tr!("cut.rows.aspect"),
        PlacementField::Rotation => tr!("cut.panel.rotation"),
        PlacementField::TiltX => tr!("cut.rows.tilt_x"),
        PlacementField::TiltY => tr!("cut.rows.tilt_y"),
        PlacementField::Opacity => tr!("cut.panel.opacity"),
    }
}

pub(crate) fn art_field(field: cut_project::ArtField, lined: bool) -> Label {
    use cut_project::ArtField;
    match field {
        ArtField::Width => tr!("cut.panel.width"),
        ArtField::Height => tr!("cut.panel.height"),
        ArtField::Corner => tr!("cut.panel.corner"),
        ArtField::Fill => tr!("cut.panel.fill_color"),
        ArtField::Stroke if lined => tr!("cut.panel.color"),
        ArtField::Stroke => tr!("cut.panel.outline_color"),
        ArtField::StrokeWidth if lined => tr!("cut.panel.line_width"),
        ArtField::StrokeWidth => tr!("cut.panel.outline_width"),
        ArtField::End => tr!("cut.rows.end"),
        ArtField::Size => tr!("cut.panel.size"),
        ArtField::Color => tr!("cut.panel.color"),
        ArtField::Spacing => tr!("cut.panel.letter_spacing"),
        ArtField::Reveal => tr!("cut.rows.reveal"),
    }
}

pub(crate) fn camera_field(field: cut_project::CameraField) -> Label {
    use cut_project::CameraField;
    match field {
        CameraField::Interest => tr!("cut.camera.interest"),
        CameraField::Dolly => tr!("cut.camera.dolly"),
        CameraField::Zoom => tr!("cut.camera.zoom"),
        CameraField::Roll => tr!("cut.camera.roll"),
        CameraField::Pan => tr!("cut.camera.pan"),
        CameraField::Tilt => tr!("cut.camera.tilt"),
        CameraField::Angle => tr!("cut.camera.angle"),
    }
}

pub(crate) fn fade_shape(shape: cut_project::FadeShape) -> Label {
    use cut_project::FadeShape;
    match shape {
        FadeShape::Linear => tr!("cut.fade.linear"),
        FadeShape::EqualPower => tr!("cut.fade.equal_power"),
        FadeShape::SCurve => tr!("cut.fade.s_curve"),
        FadeShape::Exponential => tr!("cut.fade.exponential"),
    }
}

pub(crate) fn mix_field(field: cut_project::MixField) -> Label {
    use cut_project::MixField;
    match field {
        MixField::Volume => tr!("cut.panel.volume"),
        MixField::Pan => tr!("cut.panel.pan"),
    }
}

pub(crate) fn property(property: &cut_project::Property, clip: &cut_project::Clip) -> Label {
    use cut_project::Property;
    let lined = clip
        .graphic()
        .is_some_and(|graphic| matches!(graphic.art, Art::Line { .. }));
    match property {
        Property::Placement(field) => placement_field(*field),
        Property::Art(field) => art_field(*field, lined),
        Property::Camera(field) => camera_field(*field),
        Property::Mix(field) => mix_field(*field),
        Property::Effect { slot, knob } => {
            let Some(effect_slot) = clip.effects().get(*slot) else {
                return Label::from(knob.clone());
            };
            let label = effect_slot
                .effect
                .knobs()
                .iter()
                .find(|entry| entry.key == knob)
                .map_or_else(|| knob.clone(), |entry| (entry.label)().to_string());
            tr!(
                "cut.keys.knob",
                knob = &label,
                effect = &effect_slot.effect.kind().name().to_string()
            )
        }
    }
}

pub(crate) fn preset_slot(slot: PresetSlot) -> Label {
    match slot {
        PresetSlot::In => tr!("cut.acts.enter"),
        PresetSlot::Out => tr!("cut.acts.exit"),
        PresetSlot::Loop => tr!("cut.acts.repeat"),
    }
}

pub(crate) fn animation_preset_kind(kind: AnimationPresetKind) -> Label {
    match kind {
        AnimationPresetKind::Fade => tr!("cut.acts.fade"),
        AnimationPresetKind::Slide => tr!("cut.acts.slide"),
        AnimationPresetKind::Pop => tr!("cut.acts.pop"),
        AnimationPresetKind::Zoom => tr!("cut.acts.zoom"),
        AnimationPresetKind::Spin => tr!("cut.acts.spin"),
        AnimationPresetKind::Wipe => tr!("cut.acts.wipe"),
        AnimationPresetKind::Blur => tr!("cut.acts.blur"),
        AnimationPresetKind::Typewriter => tr!("cut.acts.typewriter"),
    }
}

pub(crate) fn cycle(cycle: LoopKind) -> Label {
    match cycle {
        LoopKind::Shake => tr!("cut.acts.shake"),
        LoopKind::Pulse => tr!("cut.acts.pulse"),
        LoopKind::Float => tr!("cut.acts.float"),
        LoopKind::Spin => tr!("cut.acts.turn"),
    }
}

pub(crate) fn order(order: Order) -> Label {
    match order {
        Order::Forward => tr!("cut.acts.forward"),
        Order::Backward => tr!("cut.acts.backward"),
        Order::Centre => tr!("cut.acts.centre"),
        Order::Random(_) => tr!("cut.acts.random"),
    }
}

pub(crate) fn smooth(smooth: Smooth) -> Label {
    match smooth {
        Smooth::Manual => tr!("cut.easing.manual"),
        Smooth::Continuous => tr!("cut.easing.continuous"),
        Smooth::Auto => tr!("cut.easing.auto"),
    }
}

pub(crate) fn transition_family(family: TransitionFamily) -> Label {
    match family {
        TransitionFamily::Dissolve => tr!("cut.transition.dissolve"),
        TransitionFamily::Wipe => tr!("cut.transition.wipe"),
        TransitionFamily::Motion => tr!("cut.transition.motion"),
        TransitionFamily::Camera => tr!("cut.transition.camera"),
        TransitionFamily::Stylize => tr!("cut.transition.stylize"),
    }
}

pub(crate) fn transition_kind(kind: TransitionKind) -> Label {
    match kind {
        TransitionKind::CrossDissolve => tr!("cut.transition.cross_dissolve"),
        TransitionKind::FilmDissolve => tr!("cut.transition.film_dissolve"),
        TransitionKind::AdditiveDissolve => tr!("cut.transition.additive_dissolve"),
        TransitionKind::Dip => tr!("cut.transition.dip"),
        TransitionKind::BlurDissolve => tr!("cut.transition.blur_dissolve"),
        TransitionKind::NoiseDissolve => tr!("cut.transition.noise_dissolve"),
        TransitionKind::Wipe => tr!("cut.transition.wipe_kind"),
        TransitionKind::Split => tr!("cut.transition.split"),
        TransitionKind::IrisRound => tr!("cut.transition.iris_round"),
        TransitionKind::IrisBox => tr!("cut.transition.iris_box"),
        TransitionKind::Push => tr!("cut.transition.push"),
        TransitionKind::SlideIn => tr!("cut.transition.slide_in"),
        TransitionKind::SlideOut => tr!("cut.transition.slide_out"),
        TransitionKind::CrossZoom => tr!("cut.transition.cross_zoom"),
        TransitionKind::Turn => tr!("cut.transition.turn"),
        TransitionKind::ZoomIn => tr!("cut.transition.zoom_in"),
        TransitionKind::ZoomOut => tr!("cut.transition.zoom_out"),
        TransitionKind::Spin => tr!("cut.transition.spin"),
        TransitionKind::WhipPan => tr!("cut.transition.whip_pan"),
        TransitionKind::Stretch => tr!("cut.transition.stretch"),
        TransitionKind::Shake => tr!("cut.transition.shake"),
        TransitionKind::Glitch => tr!("cut.transition.glitch"),
        TransitionKind::ChromaSplit => tr!("cut.transition.chroma_split"),
        TransitionKind::Flash => tr!("cut.transition.flash"),
        TransitionKind::LightLeak => tr!("cut.transition.light_leak"),
        TransitionKind::FilmBurn => tr!("cut.transition.film_burn"),
        TransitionKind::Pixelate => tr!("cut.transition.pixelate"),
        TransitionKind::LensWarp => tr!("cut.transition.lens_warp"),
        TransitionKind::LiquidWarp => tr!("cut.transition.liquid_warp"),
    }
}

pub(crate) fn transition_knob(kind: TransitionKind, knob: TransitionKnob) -> Label {
    match (knob, kind) {
        (TransitionKnob::Softness, _) => tr!("cut.transition.softness"),
        (TransitionKnob::Blur, _) => tr!("cut.transition.blur"),
        (TransitionKnob::Size, _) => tr!("cut.transition.size"),
        (TransitionKnob::Shutter, _) => tr!("cut.transition.shutter"),
        (TransitionKnob::Tint, _) => tr!("cut.transition.tint"),
        (TransitionKnob::Amount, TransitionKind::ZoomIn | TransitionKind::ZoomOut) => {
            tr!("cut.transition.zoom")
        }
        (TransitionKnob::Amount, TransitionKind::Spin) => tr!("cut.transition.turns"),
        (TransitionKnob::Amount, TransitionKind::Stretch) => tr!("cut.transition.stretching"),
        (TransitionKnob::Amount, TransitionKind::Shake) => tr!("cut.transition.swing"),
        (TransitionKnob::Amount, TransitionKind::ChromaSplit) => tr!("cut.transition.split_apart"),
        (TransitionKnob::Amount, TransitionKind::Pixelate) => tr!("cut.transition.cell"),
        (TransitionKnob::Amount, TransitionKind::LensWarp | TransitionKind::LiquidWarp) => {
            tr!("cut.transition.warp")
        }
        (TransitionKnob::Amount, TransitionKind::Flash) => tr!("cut.transition.brightness"),
        (TransitionKnob::Amount, _) => tr!("cut.transition.amount"),
    }
}

pub(crate) fn alignment_at_cut(at: TransitionAlignment) -> Label {
    match at {
        TransitionAlignment::End => tr!("cut.transition.end"),
        TransitionAlignment::Centre => tr!("cut.transition.centre"),
        TransitionAlignment::Start => tr!("cut.transition.start"),
    }
}
