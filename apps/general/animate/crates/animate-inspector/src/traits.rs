use crate::Card;
use crate::Dial;
use crate::HoldDial;
use crate::Listed;
use crate::Pane;
use animate_body::Body;
use animate_body::Faces;
use animate_body::HandPreset;
use animate_body::HumanPose;
use animate_body::MorphWeights;
use animate_body::Side;
use animate_cast::Figure;
use animate_cast::FigureId;
use animate_cast::Garment;
use animate_cast::Grip;
use animate_cast::Tuning;
use animate_cloth::Field;
use animate_cloth::Knob;
use animate_cloth::Reading;
use animate_edit::Channel;
use animate_fluid::Aim;
use animate_fluid::Anchor;
use animate_fluid::Emitter;
use animate_fluid::EmitterSetting;
use animate_fluid::EmitterValue;
use animate_fluid::FluidSettings;
use animate_fluid::Property;
use animate_fluid::Shape;
use animate_hold::HoldId;
use animate_motion::Automation;
use animate_motion::GazeMode;
use animate_motion::Part;
use animate_motion::PoseSample;
use animate_motion::Tag;
use animate_solve::EyeDirection;
use rikka_app::prelude::TextDocument;
use rikka_model::mmd::VmdCurve;
use rikka_physics_tailor::AnalyzedPiece;
use rikka_physics_tailor::ClothTuning;
use rikka_physics_tailor::PieceKey;
use rikka_physics_tailor::Quality;
use std::sync::Arc;

pub trait LiquidSettings {
    fn draft_fluid_settings(&mut self, settings: FluidSettings);
    fn fluid_settings(&self) -> FluidSettings;
    fn set_fluid_settings(&mut self, settings: FluidSettings);
}

pub trait ClothEditing {
    fn can_save_cloth(&self) -> bool;
    fn choose_piece(&mut self, key: PieceKey);
    fn chosen_figure(&self) -> Option<&Figure>;
    fn chosen_piece(&self) -> Option<&AnalyzedPiece>;
    fn cloth_draft(&self, dial: Dial) -> Option<f32>;
    fn cloth_quality(&self) -> Quality;
    fn draft_cloth(&mut self, dial: Dial, value: f32);
    fn figure_tuning(&self) -> Option<ClothTuning>;
    fn garment(&self, piece: &AnalyzedPiece) -> Option<Garment>;
    fn redetect(&mut self);
    fn reset_figure_tuning(&mut self);
    fn reset_piece(&mut self, key: PieceKey);
    fn save_cloth(&mut self);
    fn set_cloth_quality(&mut self, quality: Quality);
    fn set_field(&mut self, key: PieceKey, field: Field, reading: Reading);
    fn set_figure_knob(&mut self, knob: Knob, value: f32);
    fn set_knob(&mut self, key: PieceKey, knob: Knob, value: f32);
    fn set_layer(&mut self, key: PieceKey, layer: Option<u8>);
    fn set_piece_enabled(&mut self, key: PieceKey, enabled: bool);
    fn set_preset(&mut self, key: PieceKey, preset: rikka_physics_tailor::Preset);
    fn show_painting(&mut self, open: bool);
    fn show_pane(&mut self, pane: Pane);
}

pub trait CurveEditing {
    fn channel(&self) -> Channel;
    fn channels(&self) -> &'static [Channel];
    fn choose_channel(&mut self, index: usize);
    fn shown_curve(&self) -> VmdCurve;
    fn bend(&mut self, curve: VmdCurve);
}

pub trait CardFocus {
    fn aim_gaze(&mut self, mode: GazeMode);
    fn card(&self) -> Option<Card>;
    fn eye_direction(&mut self, eye: Option<Side>, eye_direction: EyeDirection);
    fn open_card(&mut self, card: Option<Card>);
    fn shown_pose(&self, body: &Body, pose_sample: &PoseSample) -> HumanPose;
}

pub trait HoldEditing {
    fn apply_hold(&mut self, hold: HoldId, dial: HoldDial, value: f32);
    fn draft_hold(&mut self, hold: HoldId, dial: HoldDial, value: f32);
    fn drop_hold(&mut self, id: FigureId, hold: HoldId);
    fn hold_dial(&self, listed: &Listed, dial: HoldDial) -> f32;
    fn listed_holds(&self) -> Vec<Listed>;
    fn release_hold(&mut self, id: FigureId, hold: HoldId);
    fn subject_figure(&self) -> Option<FigureId>;
}

pub trait EmitterNaming {
    fn chosen_emitter(&self) -> Option<&Arc<Emitter>>;
    fn naming(&self) -> &TextDocument;
    fn rename_emitter(&mut self);
}

pub trait MorphEditing {
    fn apply_typed_morphs(&mut self);
    fn choose_morph(&mut self, morph: &str);
    fn chosen_faces(&self) -> Option<&Arc<Faces>>;
    fn chosen_morph(&self) -> Option<String>;
    fn current_morph_weights(&self) -> Option<MorphWeights>;
    fn drop_typed_morphs(&mut self);
    fn express(&mut self, change: &mut dyn FnMut(&Faces, &mut MorphWeights));
    fn morph_entry(&self) -> &TextDocument;
    fn morph_query(&self) -> &TextDocument;
    fn morphs_now(&self) -> Option<(MorphWeights, MorphWeights)>;
    fn morphs_open(&self) -> bool;
    fn revert_morphs(&mut self);
    fn set_morphs(&mut self, morphs: &[String], weight: f32);
    fn show_morphs(&mut self, open: bool);
    fn sides_open(&self, name: &str) -> bool;
    fn toggle_morph_keys(&mut self, morphs: &[String]);
    fn toggle_sides(&mut self, name: &str);
    fn type_morphs(&mut self, morphs: Vec<String>, weight: f32);
    fn typed_morphs(&self) -> Option<&[String]>;
    fn unregistered_morphs(&self) -> Vec<String>;
}

pub trait PoseEditing {
    fn apply_tuning(&mut self, tuning: Tuning, value: f32);
    fn automate(&mut self, change: &mut dyn FnMut(&mut Automation));
    fn automation(&self) -> Option<Automation>;
    fn hand_preset(&mut self, side: Side, preset: HandPreset);
    fn limits(&self) -> bool;
    fn regrip(&mut self, side: Side, grip: Grip, value: f32);
    fn reset_style(&mut self);
    fn retag(&mut self, tag: Tag);
    fn sampled_now(&self) -> Option<PoseSample>;
    fn shown_tag(&self) -> Tag;
    fn touch_with(&mut self, change: &mut dyn FnMut(&mut PoseSample));
    fn tuned(&self, tuning: Tuning) -> f32;
    fn unregistered_parts(&self) -> Vec<Part>;
}

pub trait EmitterShaping {
    fn anchor(&mut self, setting: EmitterSetting, anchor: Anchor);
    fn apply_blob(&mut self, size: f32);
    fn apply_property(&mut self, property: Property, value: EmitterValue);
    fn choose_aim(&mut self, aim: Aim);
    fn choose_preset(&mut self, preset: animate_fluid::Preset);
    fn choose_shape(&mut self, shape: Shape);
    fn choose_surface_figure(&mut self, figure: String);
    fn cover_material(&mut self, material: u32, covered: bool);
    fn draft_blob(&mut self, size: f32);
    fn draft_property(&mut self, property: Property, value: EmitterValue);
    fn mixed(&self, emitter: &Emitter, property: Property) -> EmitterValue;
    fn reset_property(&mut self, property: Property);
}

pub trait PourTiming {
    fn end_pour_here(&mut self);
    fn pour_for(&mut self, seconds: f32);
    fn pour_from(&mut self, start: u32);
    fn start_pour_here(&mut self);
}
