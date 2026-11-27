use std::path::PathBuf;
use std::sync::Arc;

use animate_cast::{Prop, PropId, PropRole, Subject};
use animate_document::PropRecord;
use animate_edit::{Change, Key, KeyId, KeyKind};
use animate_inspector::PropEditing;
use animate_motion::{Incoming, Take};
use animate_prop::{Anchor, AnchorKey, PropKey, PropKind, PropMotion, PropSetting};
use animate_sheet::Lane;
use animate_stage::{Drafts, Unkeyed};
use animate_track::Tracks;
use rikka_app::prelude::*;
use rikka_dialog::{Chooser, ChooserResponse, Filter};
use rikka_model::Model;
use rikka_model::mmd::VmdCurve;
use rikka_model_gpu::{Prepared, UploadOptions};
use rustc_hash::FxHashSet;

use super::{Animate, Batch, FIGURES, SCENES};
use crate::animate::settings::SettingSubject;
use crate::animate::subject_motion::SubjectMotion;
use crate::arrival::{Placement, Rejection};
use crate::wording;

impl SettingSubject for PropRole {
    type Motion = Prop;
    type Setting = PropSetting;
    type Value = f32;
    type Key = PropKey;

    const KIND: KeyKind = KeyKind::Prop;

    fn subject(self) -> Subject {
        PropRole::subject(self)
    }

    fn tracks(motion: &Prop) -> &Tracks<PropKey> {
        motion.motion.tracks()
    }

    fn value_at(motion: &Prop, setting: PropSetting, frame: f32) -> f32 {
        motion.motion.value_at(setting, frame)
    }

    fn rest(motion: &mut Prop, setting: PropSetting, value: f32) {
        motion.motion.levels.set(setting, value);
    }

    fn rows(_: &Prop) -> Vec<PropSetting> {
        PropSetting::ALL.to_vec()
    }

    fn lane_setting(lane: &Lane) -> Option<PropSetting> {
        match lane {
            Lane::Prop(setting) => Some(*setting),
            _ => None,
        }
    }

    fn key(setting: PropSetting, frame: u32, value: f32, curve: VmdCurve) -> Key {
        Key::Prop(PropKey {
            setting,
            frame,
            value,
            curve,
        })
    }

    fn curve_of(key: &PropKey) -> VmdCurve {
        key.curve
    }

    fn held(drafts: &Drafts) -> &Unkeyed<Self, PropSetting, f32> {
        &drafts.props
    }

    fn held_mut(drafts: &mut Drafts) -> &mut Unkeyed<Self, PropSetting, f32> {
        &mut drafts.props
    }
}

impl Animate {
    pub(crate) fn place_stage(&mut self, path: PathBuf) {
        self.load_as(vec![path], Placement::Stage);
    }

    pub(crate) fn add_accessory(&mut self, path: PathBuf) {
        self.load_as(vec![path], Placement::Accessory);
    }

    pub(crate) fn choose_stage(&self) {
        self.choose_props(PropKind::Stage);
    }

    pub(crate) fn choose_accessories(&self) {
        self.choose_props(PropKind::Accessory);
    }

    fn choose_props(&self, kind: PropKind) {
        let (title, multiple) = match kind {
            PropKind::Stage => (tr!("animate.add.stage_title"), false),
            PropKind::Accessory => (tr!("animate.add.accessory_title"), true),
        };
        let models = FIGURES.into_iter().chain(SCENES).fold(
            Filter::named(tr!("animate.add.figure_files").to_string()),
            Filter::mime_type,
        );
        let chooser = Chooser::open(title.to_string())
            .parent(window_identifier())
            .accept_label(tr!("animate.add.accept").to_string())
            .multiple(multiple)
            .filter(models);
        let chooser = match &self.folder {
            Some(folder) => chooser.current_folder(folder),
            None => chooser,
        };
        spawn_blocking(
            move || chooser.run(),
            move |animate: &mut Animate, chosen: rikka_dialog::Result<ChooserResponse>| match chosen
            {
                Ok(ChooserResponse::Files(paths)) => {
                    animate.folder = paths
                        .first()
                        .and_then(|path| path.parent())
                        .map(PathBuf::from);
                    for path in paths {
                        match kind {
                            PropKind::Stage => animate.place_stage(path),
                            PropKind::Accessory => animate.add_accessory(path),
                        }
                    }
                }
                Ok(ChooserResponse::Cancelled) => skip_view(),
                Err(error) => animate.notice = Some(Label::from(error.to_string())),
            },
        );
    }

    pub(super) fn load_as(&mut self, paths: Vec<PathBuf>, placement: Placement) {
        if paths.is_empty() {
            return;
        }
        self.loading.queue.push_back(Batch {
            paths,
            settled: None,
            placement,
        });
        self.next_load();
    }

    pub(super) fn sync_props(&mut self) {
        let (lacking, uploads): (Vec<(PropId, Arc<Model>)>, _) = {
            let mut stage = self.stage.lock();
            stage.set_props(self.cast.props());
            let lacking = self
                .cast
                .props()
                .map(|track| &track.prop)
                .filter(|prop| stage.lacks_prop(prop.id) && !self.preparing.contains(&prop.id))
                .map(|prop| (prop.id, Arc::clone(&prop.model)))
                .collect();
            (lacking, stage.uploads())
        };
        for (id, model) in lacking {
            self.preparing.insert(id);
            let uploads = uploads.clone();
            spawn_blocking(
                move || Prepared::with_uploads(model, &UploadOptions::default(), uploads.as_ref()),
                move |animate: &mut Animate, prepared| animate.on_prop_prepared(id, prepared),
            );
        }
    }

    fn on_prop_prepared(&mut self, id: PropId, prepared: Prepared) {
        self.preparing.remove(&id);
        if self.cast.props().any(|track| track.prop.id == id) {
            self.stage.lock().supply_prop(id, prepared);
        }
        skip_view();
    }

    pub(super) fn accept_prop(&mut self, prop: Prop, prepared: Prepared) {
        let subject = prop.role().subject();
        self.stage.lock().supply_prop(prop.id, prepared);
        self.replace(subject, Some(SubjectMotion::Prop(Arc::new(prop))));
        self.subject = Some(subject);
        self.timeline.pick(FxHashSet::default());
    }

    pub(super) fn land_prop_take(
        &mut self,
        role: PropRole,
        onto: &Arc<Take>,
        incoming: &Incoming<Take>,
        take: Arc<Take>,
    ) {
        let subject = role.subject();
        let Some(track) = self.cast.prop(subject) else {
            return;
        };
        let mut prop = (*track.prop).clone();
        prop.take = match Arc::ptr_eq(&prop.take, onto) {
            true => take,
            false => Arc::new(incoming.onto(&prop.take)),
        };
        self.replace(subject, Some(SubjectMotion::Prop(Arc::new(prop))));
    }

    pub(super) fn land_placement(&mut self, file: &str, kind: PropKind, record: PropRecord) {
        let subject = match kind {
            PropKind::Stage => self.cast.stage().map(|_| Subject::Stage),
            PropKind::Accessory => self
                .subject
                .filter(|subject| matches!(subject, Subject::Accessory(_)))
                .filter(|subject| self.cast.prop(*subject).is_some()),
        };
        let Some(subject) = subject else {
            let rejection = match kind {
                PropKind::Stage => Rejection::NeedsStage,
                PropKind::Accessory => Rejection::NeedsAccessory,
            };
            self.notice = Some(wording::rejection(file, &rejection));
            return;
        };
        let PropRecord { motion, take } = record;
        self.update::<Prop>(subject, |prop| {
            prop.motion = motion;
            prop.take = Arc::new(take);
        });
    }

    pub(crate) fn clear_prop_motion(&mut self, subject: Subject) {
        self.update::<Prop>(subject, |prop| {
            let mut cleared = PropMotion::default();
            cleared.levels = prop.motion.levels;
            cleared.anchor = prop.motion.anchor.clone();
            prop.motion = cleared;
            prop.take = Arc::default();
        });
    }

    fn lay_anchor(&mut self, role: PropRole, anchor: Option<Anchor>) {
        let frame = self.transport.frame();
        self.lay(
            role.subject(),
            vec![Key::Anchor(AnchorKey { frame, anchor })],
        );
    }

    pub(crate) fn arrange(&mut self, role: PropRole, settings: &[(PropSetting, f32)]) {
        self.in_undo_group(|animate| {
            for (setting, value) in settings {
                let range = setting.range();
                animate.apply(role, *setting, value.clamp(*range.start(), *range.end()));
            }
        });
    }

    pub(super) fn register_prop(&mut self, role: PropRole, frame: u32) {
        let anchors = matches!(self.timeline.chosen_lane(), Some(Lane::Anchor));
        self.in_undo_group(|animate| {
            animate.register_settings(role, frame);
            if anchors {
                animate.key_anchor(role);
            }
        });
    }
}

impl PropEditing for Animate {
    fn prop_of(&self, role: PropRole) -> Option<Arc<Prop>> {
        self.cast
            .prop(role.subject())
            .map(|track| Arc::clone(&track.prop))
    }

    fn prop_value(&self, role: PropRole, prop: &Prop, setting: PropSetting) -> f32 {
        self.setting(role, prop, setting)
    }

    fn apply_prop(&mut self, role: PropRole, setting: PropSetting, value: f32) {
        self.apply(role, setting, value);
    }

    fn key_prop(&mut self, role: PropRole, setting: PropSetting) {
        self.key_setting(role, setting);
    }

    fn clear_prop(&mut self, role: PropRole, settings: &[PropSetting]) {
        self.clear_settings(role, settings);
    }

    fn attach_prop(&mut self, role: PropRole, anchor: Option<Anchor>) {
        let Some(prop) = self.prop_of(role) else {
            return;
        };
        match prop.motion.anchors().is_empty() {
            true => self.update::<Prop>(role.subject(), |prop| prop.motion.anchor = anchor),
            false => self.lay_anchor(role, anchor),
        }
    }

    fn key_anchor(&mut self, role: PropRole) {
        let Some(prop) = self.prop_of(role) else {
            return;
        };
        let frame = self.transport.frame();
        let anchor = prop.motion.anchor_at(frame as f32).cloned();
        self.lay_anchor(role, anchor);
    }

    fn clear_anchor(&mut self, role: PropRole) {
        let Some(prop) = self.prop_of(role) else {
            return;
        };
        let ids: FxHashSet<KeyId> = prop
            .motion
            .anchors()
            .keys(&())
            .iter()
            .map(|key| KeyId::new(Lane::Anchor, key.frame))
            .collect();
        self.commit(role.subject(), Change::deleting(&prop, &ids));
    }
}
