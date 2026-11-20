use std::sync::Arc;

use animate_fluid::{Emitter, EmitterSetting, EmitterValue, ValueKind};
use animate_ui::{framed, key_controls, labelled};
use rikka_app::prelude::*;

use crate::inspector::liquid::timing;
use crate::{Desk, Reach};

pub(super) fn card(desk: Desk<'_>, emitter: &Arc<Emitter>) -> Element {
    let frame = desk.transport().frame();
    let mut parts = timing::rows(desk, emitter);
    parts.extend(
        emitter
            .rows()
            .map(|setting| row(desk, emitter, setting, frame)),
    );
    framed(parts)
}

fn keys(reach: Reach, emitter: &Emitter, setting: EmitterSetting, frame: u32) -> Element {
    let id = emitter.id;
    key_controls(
        emitter.tracks().key(&setting, frame).is_some(),
        !emitter.tracks().keys(&setting).is_empty(),
        reach.act(move |studio| studio.key_emitter(id, setting)),
        reach.act(move |studio| studio.clear_emitter(id, &[setting])),
    )
}

fn row(desk: Desk<'_>, emitter: &Emitter, setting: EmitterSetting, frame: u32) -> Element {
    let control = match setting.kind() {
        ValueKind::Amount { min, max } | ValueKind::Angle { min, max } => {
            let id = emitter.id;
            let value = desk
                .emitter_value(id, emitter, setting)
                .number()
                .unwrap_or(min)
                .clamp(min, max);
            slider(value)
                .range(min..=max)
                .control_size(ControlSize::Small)
                .on_change(desk.act_with(move |studio, value: f32| {
                    studio.draft_emitter(id, setting, EmitterValue::Number(value));
                }))
                .on_commit(desk.act_with(move |studio, value: f32| {
                    studio.apply_emitter(id, setting, EmitterValue::Number(value));
                }))
                .fill_width()
                .any()
        }
        ValueKind::Position | ValueKind::Color => {
            let unkeyed = desk.unkeyed_emitter(emitter.id, setting).is_some();
            let note = match unkeyed {
                true => tr!("animate.easy.unregistered"),
                false => Label::default(),
            };
            text(note).caption().muted().no_wrap().any()
        }
    };
    labelled(
        animate_ui::emitter_setting_label(setting),
        control,
        Some(keys(desk.reach(), emitter, setting, frame)),
    )
}
