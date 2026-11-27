use animate_cast::{Cast, Prop, PropRole};
use animate_prop::{Anchor, PropSetting};
use animate_ui::{framed, key_controls, labelled};
use rikka_app::prelude::*;

use crate::{Desk, Reach};

const NUMBERS: [PropSetting; 7] = [
    PropSetting::X,
    PropSetting::Y,
    PropSetting::Z,
    PropSetting::Pitch,
    PropSetting::Yaw,
    PropSetting::Roll,
    PropSetting::Scale,
];

pub(super) fn prop(desk: Desk<'_>, role: PropRole) -> Option<Element> {
    let prop = desk.prop_of(role)?;
    let frame = desk.transport().frame();
    let mut parts = vec![text(prop.name.as_str()).bold().no_wrap().any()];
    parts.extend(
        NUMBERS
            .into_iter()
            .map(|setting| number(desk, role, &prop, setting, frame)),
    );
    parts.push(visible(desk, role, &prop, frame));
    if prop.kind.attaches() {
        let anchor = prop.motion.anchor_at(frame as f32).cloned();
        parts.push(figure_row(desk, role, &prop, anchor.as_ref(), frame));
        if let Some(anchor) = anchor {
            parts.push(bone_row(desk, role, &anchor));
        }
    }
    Some(framed(parts))
}

fn keys(reach: Reach, role: PropRole, prop: &Prop, setting: PropSetting, frame: u32) -> Element {
    let tracks = prop.motion.tracks();
    key_controls(
        tracks.key(&setting, frame).is_some(),
        !tracks.keys(&setting).is_empty(),
        reach.act(move |studio| studio.key_prop(role, setting)),
        reach.act(move |studio| studio.clear_prop(role, &[setting])),
    )
}

fn number(
    desk: Desk<'_>,
    role: PropRole,
    prop: &Prop,
    setting: PropSetting,
    frame: u32,
) -> Element {
    let range = setting.range();
    let (step, decimals) = match setting {
        PropSetting::Pitch | PropSetting::Yaw | PropSetting::Roll => (1.0, 1),
        _ => (0.01, 3),
    };
    let value = desk
        .prop_value(role, prop, setting)
        .clamp(*range.start(), *range.end());
    labelled(
        animate_ui::prop_setting_label(setting),
        number_field(value)
            .range(range)
            .step(step)
            .decimals(decimals)
            .control_size(ControlSize::Small)
            .on_commit(desk.act_with(move |studio, value: f32| {
                studio.apply_prop(role, setting, value);
            }))
            .fill_width(),
        Some(keys(desk.reach(), role, prop, setting, frame)),
    )
}

fn visible(desk: Desk<'_>, role: PropRole, prop: &Prop, frame: u32) -> Element {
    let shown = desk.prop_value(role, prop, PropSetting::Visible) >= 0.5;
    labelled(
        animate_ui::prop_setting_label(PropSetting::Visible),
        switch(shown)
            .control_size(ControlSize::Small)
            .on_toggle(desk.act_with(move |studio, shown: bool| {
                studio.apply_prop(role, PropSetting::Visible, f32::from(u8::from(shown)));
            })),
        Some(keys(desk.reach(), role, prop, PropSetting::Visible, frame)),
    )
}

fn figure_row(
    desk: Desk<'_>,
    role: PropRole,
    prop: &Prop,
    anchor: Option<&Anchor>,
    frame: u32,
) -> Element {
    let figures = desk.cast().figures();
    let chosen = anchor
        .and_then(|anchor| {
            figures
                .iter()
                .position(|figure| figure.name == anchor.figure)
        })
        .map_or(0, |at| at + 1);
    let options = std::iter::once(tr!("animate.placement.world")).chain(
        figures
            .iter()
            .map(|figure| Label::from(figure.name.as_str())),
    );
    let anchors = prop.motion.anchors();
    let reach = desk.reach();
    let anchor_bone = anchor.map(|anchor| anchor.bone.clone());
    labelled(
        animate_ui::anchor_label(),
        combo(Some(chosen))
            .options(options)
            .on_select(desk.act_with(move |studio, index: usize| {
                let held = anchor_bone.as_deref();
                let anchor = attached_to(studio.cast(), index, held);
                studio.attach_prop(role, anchor);
            }))
            .focusless()
            .control_size(ControlSize::Small)
            .fill_width(),
        Some(key_controls(
            anchors.key(&(), frame).is_some(),
            !anchors.is_empty(),
            reach.act(move |studio| studio.key_anchor(role)),
            reach.act(move |studio| studio.clear_anchor(role)),
        )),
    )
}

fn bone_row(desk: Desk<'_>, role: PropRole, anchor: &Anchor) -> Element {
    let Some(figure) = desk
        .cast()
        .figures()
        .iter()
        .find(|figure| figure.name == anchor.figure)
    else {
        return labelled(
            tr!("animate.placement.bone"),
            text(anchor.bone.as_str()).caption().no_wrap(),
            None,
        );
    };
    let bones = figure.skeleton.bones();
    let chosen = bones.iter().position(|bone| bone.name == anchor.bone);
    let figure_name = anchor.figure.clone();
    labelled(
        tr!("animate.placement.bone"),
        combo(chosen)
            .options(bones.iter().map(|bone| Label::from(bone.name.as_str())))
            .on_select(desk.act_with(move |studio, index: usize| {
                let bone = studio
                    .cast()
                    .figures()
                    .iter()
                    .find(|figure| figure.name == figure_name)
                    .and_then(|figure| figure.skeleton.bone(index))
                    .map(|bone| bone.name.clone());
                if let Some(bone) = bone {
                    studio.attach_prop(
                        role,
                        Some(Anchor {
                            figure: figure_name.clone(),
                            bone,
                        }),
                    );
                }
            }))
            .focusless()
            .control_size(ControlSize::Small)
            .fill_width(),
        None,
    )
}

fn attached_to(cast: &Cast, index: usize, held: Option<&str>) -> Option<Anchor> {
    let figure = cast.figures().get(index.checked_sub(1)?)?;
    let bones = figure.skeleton.bones();
    let bone = held
        .and_then(|held| bones.iter().find(|bone| bone.name == held))
        .or_else(|| bones.first())?;
    Some(Anchor {
        figure: figure.name.clone(),
        bone: bone.name.clone(),
    })
}
