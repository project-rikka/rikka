use std::collections::BTreeMap;
use std::fmt::Debug;
use std::sync::Arc;

use animate_cast::Subject;
use animate_edit::{Change, Key, KeyId, KeyKind, Keyed};
use animate_fluid::{Emitter, EmitterId, EmitterKey, EmitterSetting, EmitterValue};
use animate_inspector::SettingEditing;
use animate_sheet::Lane;
use animate_stage::{Blowing, Drafts, Unkeyed};
use animate_track::{Keyframe, Sampled, Tracks};
use animate_wind::{WindKey, WindMotion, WindSetting};
use rikka_keyframe::Framed;
use rikka_model::mmd::VmdCurve;
use rustc_hash::FxHashSet;

use crate::animate::Animate;
use crate::animate::subject_motion::Subjective;

pub(crate) trait SettingSubject: Copy + PartialEq + Debug + 'static {
    type Motion: Subjective + Keyed;
    type Setting: Copy + Ord + Debug;
    type Value: Copy + PartialEq;
    type Key: Sampled<Id = Self::Setting, Value = Self::Value>;

    const KIND: KeyKind;

    fn subject(self) -> Subject;

    fn tracks(motion: &Self::Motion) -> &Tracks<Self::Key>;

    fn value_at(motion: &Self::Motion, setting: Self::Setting, frame: f32) -> Self::Value;

    fn rest(motion: &mut Self::Motion, setting: Self::Setting, value: Self::Value);

    fn rows(motion: &Self::Motion) -> Vec<Self::Setting>;

    fn lane_setting(lane: &Lane) -> Option<Self::Setting>;

    fn key(setting: Self::Setting, frame: u32, value: Self::Value, curve: VmdCurve) -> Key;

    fn curve_of(key: &Self::Key) -> VmdCurve;

    fn held(drafts: &Drafts) -> &Unkeyed<Self, Self::Setting, Self::Value>;

    fn held_mut(drafts: &mut Drafts) -> &mut Unkeyed<Self, Self::Setting, Self::Value>;
}

impl SettingSubject for Blowing {
    type Motion = WindMotion;
    type Setting = WindSetting;
    type Value = f32;
    type Key = WindKey;

    const KIND: KeyKind = KeyKind::Wind;

    fn subject(self) -> Subject {
        Subject::Wind
    }

    fn tracks(motion: &WindMotion) -> &Tracks<WindKey> {
        motion.tracks()
    }

    fn value_at(motion: &WindMotion, setting: WindSetting, frame: f32) -> f32 {
        motion.value_at(setting, frame)
    }

    fn rest(motion: &mut WindMotion, setting: WindSetting, value: f32) {
        motion.levels.set(setting, value);
    }

    fn rows(_: &WindMotion) -> Vec<WindSetting> {
        WindSetting::ALL.to_vec()
    }

    fn lane_setting(lane: &Lane) -> Option<WindSetting> {
        match lane {
            Lane::Wind(setting) => Some(*setting),
            _ => None,
        }
    }

    fn key(setting: WindSetting, frame: u32, value: f32, curve: VmdCurve) -> Key {
        Key::Wind(WindKey {
            setting,
            frame,
            value,
            curve,
        })
    }

    fn curve_of(key: &WindKey) -> VmdCurve {
        key.curve
    }

    fn held(drafts: &Drafts) -> &Unkeyed<Self, WindSetting, f32> {
        &drafts.wind
    }

    fn held_mut(drafts: &mut Drafts) -> &mut Unkeyed<Self, WindSetting, f32> {
        &mut drafts.wind
    }
}

impl SettingSubject for EmitterId {
    type Motion = Emitter;
    type Setting = EmitterSetting;
    type Value = EmitterValue;
    type Key = EmitterKey;

    const KIND: KeyKind = KeyKind::Fluid;

    fn subject(self) -> Subject {
        Subject::Emitter(self)
    }

    fn tracks(motion: &Emitter) -> &Tracks<EmitterKey> {
        motion.tracks()
    }

    fn value_at(motion: &Emitter, setting: EmitterSetting, frame: f32) -> EmitterValue {
        motion.value_at(setting, frame)
    }

    fn rest(motion: &mut Emitter, setting: EmitterSetting, value: EmitterValue) {
        motion.set_resting(setting, value);
    }

    fn rows(motion: &Emitter) -> Vec<EmitterSetting> {
        motion.rows().collect()
    }

    fn lane_setting(lane: &Lane) -> Option<EmitterSetting> {
        match lane {
            Lane::Fluid(setting) => Some(*setting),
            _ => None,
        }
    }

    fn key(setting: EmitterSetting, frame: u32, value: EmitterValue, curve: VmdCurve) -> Key {
        Key::Fluid(EmitterKey {
            setting,
            frame,
            value,
            curve,
        })
    }

    fn curve_of(key: &EmitterKey) -> VmdCurve {
        key.curve
    }

    fn held(drafts: &Drafts) -> &Unkeyed<Self, EmitterSetting, EmitterValue> {
        &drafts.pour
    }

    fn held_mut(drafts: &mut Drafts) -> &mut Unkeyed<Self, EmitterSetting, EmitterValue> {
        &mut drafts.pour
    }
}

impl Animate {
    pub(crate) fn settings_of<S: SettingSubject>(&self, target: S) -> Option<Arc<S::Motion>> {
        S::Motion::of(&self.motion_of(target.subject())?).cloned()
    }

    pub(crate) fn has_unkeyed<S: SettingSubject>(&self, target: S) -> bool {
        S::held(self.stage.lock().drafts())
            .at(target, self.transport.frame())
            .is_some_and(|held| !held.is_empty())
    }

    pub(crate) fn unkeyed<S: SettingSubject>(
        &self,
        target: S,
        setting: S::Setting,
    ) -> Option<S::Value> {
        S::held(self.stage.lock().drafts())
            .at(target, self.transport.frame())
            .and_then(|held| held.get(setting))
    }

    pub(crate) fn setting<S: SettingSubject>(
        &self,
        target: S,
        motion: &S::Motion,
        setting: S::Setting,
    ) -> S::Value {
        let frame = self.transport.frame();
        self.unkeyed(target, setting)
            .unwrap_or_else(|| S::value_at(motion, setting, frame as f32))
    }

    pub(crate) fn draft<S: SettingSubject>(
        &mut self,
        target: S,
        setting: S::Setting,
        value: S::Value,
    ) {
        let frame = self.transport.frame();
        S::held_mut(self.stage.lock().drafts_mut()).set(target, frame, setting, value);
    }

    pub(crate) fn apply<S: SettingSubject>(
        &mut self,
        target: S,
        setting: S::Setting,
        value: S::Value,
    ) {
        let Some(motion) = self.settings_of(target) else {
            return;
        };
        if S::tracks(&motion).keys(&setting).is_empty() {
            S::held_mut(self.stage.lock().drafts_mut()).clear(target, setting);
            self.update::<S::Motion>(target.subject(), |motion| S::rest(motion, setting, value));
            return;
        }
        let frame = self.transport.frame();
        let mut stage = self.stage.lock();
        let held = S::held_mut(stage.drafts_mut());
        match S::value_at(&motion, setting, frame as f32) == value {
            true => {
                held.clear(target, setting);
            }
            false => held.set(target, frame, setting, value),
        }
    }

    pub(crate) fn key_setting<S: SettingSubject>(&mut self, target: S, setting: S::Setting) {
        let Some(motion) = self.settings_of(target) else {
            return;
        };
        let key = self.setting_key::<S>(&motion, setting, self.setting(target, &motion, setting));
        S::held_mut(self.stage.lock().drafts_mut()).clear(target, setting);
        self.lay(target.subject(), vec![key]);
    }

    pub(crate) fn clear_settings<S: SettingSubject>(&mut self, target: S, settings: &[S::Setting]) {
        let Some(motion) = self.settings_of(target) else {
            return;
        };
        let ids: FxHashSet<KeyId> = settings
            .iter()
            .flat_map(|setting| S::tracks(&motion).keys(setting))
            .map(|key| S::key(*key.id(), key.frame(), key.value(), S::curve_of(key)).id())
            .collect();
        let mut stage = self.stage.lock();
        let held = S::held_mut(stage.drafts_mut());
        for setting in settings {
            held.clear(target, *setting);
        }
        drop(stage);
        self.commit(target.subject(), Change::deleting(&motion, &ids));
    }

    pub(super) fn register_settings<S: SettingSubject>(&mut self, target: S, frame: u32) {
        let Some(motion) = self.settings_of(target) else {
            return;
        };
        let held = S::held_mut(self.stage.lock().drafts_mut())
            .take_at(target, frame)
            .flatten()
            .map(|held| held.into_values())
            .unwrap_or_default();
        let chosen = self
            .timeline
            .chosen_lane()
            .and_then(|lane| S::lane_setting(&lane));
        let rows = S::rows(&motion);
        let keys = registered(
            held,
            chosen,
            || rows.clone(),
            |setting| S::value_at(&motion, setting, frame as f32),
        )
        .into_iter()
        .filter(|(setting, _)| rows.contains(setting))
        .map(|(setting, value)| self.setting_key::<S>(&motion, setting, value))
        .collect();
        self.lay(target.subject(), keys);
    }

    fn setting_key<S: SettingSubject>(
        &self,
        motion: &S::Motion,
        setting: S::Setting,
        value: S::Value,
    ) -> Key {
        let frame = self.transport.frame();
        let curve = S::tracks(motion)
            .key(&setting, frame)
            .map_or(self.curves.value(S::KIND), S::curve_of);
        S::key(setting, frame, value, curve)
    }
}

fn registered<S: Copy + Ord, V: Copy>(
    held: BTreeMap<S, V>,
    chosen: Option<S>,
    every: impl FnOnce() -> Vec<S>,
    value_at: impl Fn(S) -> V,
) -> Vec<(S, V)> {
    let mut settings: Vec<S> = held.keys().copied().collect();
    if let Some(chosen) = chosen
        && !settings.contains(&chosen)
    {
        settings.push(chosen);
    }
    if settings.is_empty() {
        settings = every();
    }
    settings
        .into_iter()
        .map(|setting| {
            let value = held
                .get(&setting)
                .copied()
                .unwrap_or_else(|| value_at(setting));
            (setting, value)
        })
        .collect()
}

impl SettingEditing for Animate {
    fn wind_motion(&self) -> Arc<WindMotion> {
        Animate::wind_motion(self)
    }

    fn wind_value(&self, motion: &WindMotion, setting: WindSetting) -> f32 {
        self.setting(Blowing, motion, setting)
    }

    fn draft_wind(&mut self, setting: WindSetting, value: f32) {
        self.draft(Blowing, setting, value);
    }

    fn apply_wind(&mut self, setting: WindSetting, value: f32) {
        self.apply(Blowing, setting, value);
    }

    fn key_wind(&mut self, setting: WindSetting) {
        self.key_setting(Blowing, setting);
    }

    fn clear_wind(&mut self, settings: &[WindSetting]) {
        self.clear_settings(Blowing, settings);
    }

    fn emitter_value(
        &self,
        emitter: EmitterId,
        motion: &Emitter,
        setting: EmitterSetting,
    ) -> EmitterValue {
        self.setting(emitter, motion, setting)
    }

    fn unkeyed_emitter(&self, emitter: EmitterId, setting: EmitterSetting) -> Option<EmitterValue> {
        self.unkeyed(emitter, setting)
    }

    fn draft_emitter(&mut self, emitter: EmitterId, setting: EmitterSetting, value: EmitterValue) {
        self.draft(emitter, setting, value);
    }

    fn apply_emitter(&mut self, emitter: EmitterId, setting: EmitterSetting, value: EmitterValue) {
        self.apply(emitter, setting, value);
    }

    fn key_emitter(&mut self, emitter: EmitterId, setting: EmitterSetting) {
        self.key_setting(emitter, setting);
    }

    fn clear_emitter(&mut self, emitter: EmitterId, settings: &[EmitterSetting]) {
        self.clear_settings(emitter, settings);
    }
}
