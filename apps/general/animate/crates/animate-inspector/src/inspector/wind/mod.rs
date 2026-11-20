mod compass;

use animate_ui::{framed, key_controls, labelled};
use animate_wind::{WindMotion, WindSetting};
use rikka_app::prelude::*;

use crate::{Desk, Reach};
use compass::Compass;

const COMPASS: f32 = 132.0;
const SLIDERS: [WindSetting; 4] = [
    WindSetting::Pitch,
    WindSetting::Speed,
    WindSetting::Turbulence,
    WindSetting::Gust,
];

pub(super) fn wind(desk: Desk<'_>) -> Element {
    let motion = desk.wind_motion();
    let frame = desk.transport().frame();
    let mut parts = vec![
        text(tr!("animate.wind.title")).bold().no_wrap().any(),
        labelled(
            animate_ui::wind_setting_label(WindSetting::Yaw),
            compass(desk),
            Some(keys(desk.reach(), &motion, WindSetting::Yaw, frame)),
        ),
    ];
    parts.extend(
        SLIDERS
            .into_iter()
            .map(|setting| row(desk, &motion, setting, frame)),
    );
    framed(parts)
}

fn compass(desk: Desk<'_>) -> Element {
    Element::new(Compass::new(
        desk.wind_value(&desk.wind_motion(), WindSetting::Yaw),
        tr!("animate.wind.front"),
        desk.act_with(move |studio, yaw: f32| studio.draft_wind(WindSetting::Yaw, yaw)),
        desk.act_with(move |studio, yaw: f32| studio.apply_wind(WindSetting::Yaw, yaw)),
    ))
    .width(COMPASS)
    .height(COMPASS)
}

fn keys(reach: Reach, motion: &WindMotion, setting: WindSetting, frame: u32) -> Element {
    key_controls(
        motion.tracks().key(&setting, frame).is_some(),
        !motion.tracks().keys(&setting).is_empty(),
        reach.act(move |studio| studio.key_wind(setting)),
        reach.act(move |studio| studio.clear_wind(&[setting])),
    )
}

fn row(desk: Desk<'_>, motion: &WindMotion, setting: WindSetting, frame: u32) -> Element {
    let range = setting.range();
    let value = desk
        .wind_value(motion, setting)
        .clamp(*range.start(), *range.end());
    labelled(
        animate_ui::wind_setting_label(setting),
        slider(value)
            .range(range)
            .control_size(ControlSize::Small)
            .on_change(desk.act_with(move |studio, value: f32| {
                studio.draft_wind(setting, value);
            }))
            .on_commit(desk.act_with(move |studio, value: f32| {
                studio.apply_wind(setting, value);
            }))
            .fill_width()
            .any(),
        Some(keys(desk.reach(), motion, setting, frame)),
    )
}
