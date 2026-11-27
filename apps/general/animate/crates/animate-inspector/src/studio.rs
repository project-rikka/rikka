use std::sync::Arc;

use animate_body::Body;
use animate_cast::{Cast, Mode, Subject};
use animate_fluid::{Emitter, EmitterId, EmitterSetting, EmitterValue};
use animate_transport::Transport;
use animate_wind::{WindMotion, WindSetting};

use crate::controls::Controls;
use crate::prop_editing::PropEditing;
use crate::traits::{
    CardFocus, ClothEditing, CurveEditing, EmitterNaming, EmitterShaping, HoldEditing,
    LiquidSettings, MorphEditing, PoseEditing, PourTiming,
};

pub trait StudioCore {
    fn cast(&self) -> &Cast;
    fn chosen_body(&self) -> Option<&Arc<Body>>;
    fn controls(&self) -> &Controls;
    fn controls_mut(&mut self) -> &mut Controls;
    fn is_easy_mode(&self) -> bool;
    fn is_physics_enabled(&self) -> bool;
    fn mode(&self) -> Mode;
    fn reset_physics(&mut self);
    fn set_physics(&mut self, physics: bool);
    fn subject(&self) -> Option<Subject>;
    fn transport(&self) -> &Transport;
}

pub trait SettingEditing {
    fn wind_motion(&self) -> Arc<WindMotion>;
    fn wind_value(&self, motion: &WindMotion, setting: WindSetting) -> f32;
    fn draft_wind(&mut self, setting: WindSetting, value: f32);
    fn apply_wind(&mut self, setting: WindSetting, value: f32);
    fn key_wind(&mut self, setting: WindSetting);
    fn clear_wind(&mut self, settings: &[WindSetting]);
    fn emitter_value(
        &self,
        emitter: EmitterId,
        motion: &Emitter,
        setting: EmitterSetting,
    ) -> EmitterValue;
    fn unkeyed_emitter(&self, emitter: EmitterId, setting: EmitterSetting) -> Option<EmitterValue>;
    fn draft_emitter(&mut self, emitter: EmitterId, setting: EmitterSetting, value: EmitterValue);
    fn apply_emitter(&mut self, emitter: EmitterId, setting: EmitterSetting, value: EmitterValue);
    fn key_emitter(&mut self, emitter: EmitterId, setting: EmitterSetting);
    fn clear_emitter(&mut self, emitter: EmitterId, settings: &[EmitterSetting]);
}

pub trait Studio:
    StudioCore
    + SettingEditing
    + CardFocus
    + ClothEditing
    + CurveEditing
    + EmitterNaming
    + EmitterShaping
    + HoldEditing
    + LiquidSettings
    + MorphEditing
    + PoseEditing
    + PourTiming
    + PropEditing
{
}

impl<T> Studio for T where
    T: StudioCore
        + SettingEditing
        + CardFocus
        + ClothEditing
        + CurveEditing
        + EmitterNaming
        + EmitterShaping
        + HoldEditing
        + LiquidSettings
        + MorphEditing
        + PoseEditing
        + PourTiming
        + PropEditing
{
}

pub type Desk<'a> = animate_ui::Desk<'a, dyn Studio>;

pub type Reach = animate_ui::Reach<dyn Studio>;
