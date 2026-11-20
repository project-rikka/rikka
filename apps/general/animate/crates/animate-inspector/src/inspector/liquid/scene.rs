use animate_fluid::{FluidSettings, Gravity, Wetness};
use animate_ui::{framed, heading, labelled};
use rikka_app::prelude::*;
use rikka_model::glam::Vec3;

use crate::{Desk, Reach};

const CAPACITY_STEP: f32 = 10_000.0;
const STRONGEST_PULL: f32 = 20.0;

pub(super) fn card(desk: Desk<'_>) -> Element {
    let settings = desk.fluid_settings();
    let mut parts = vec![
        heading(tr!("animate.liquid.scene")),
        labelled(
            tr!("animate.liquid.particle_size"),
            tuned(
                desk.reach(),
                settings.particle_size,
                FluidSettings::PARTICLE_SIZES,
                0.0,
                |settings, size| settings.particle_size = size,
            ),
            None,
        ),
        labelled(
            tr!("animate.liquid.capacity"),
            tuned(
                desk.reach(),
                settings.capacity as f32,
                *FluidSettings::CAPACITIES.start() as f32..=*FluidSettings::CAPACITIES.end() as f32,
                CAPACITY_STEP,
                |settings, capacity| settings.capacity = capacity.round() as u32,
            ),
            None,
        ),
        labelled(
            tr!("animate.liquid.gravity"),
            tuned(
                desk.reach(),
                -settings.gravity.vector().y,
                0.0..=STRONGEST_PULL,
                0.0,
                |settings, pull| settings.gravity = pulled(pull),
            ),
            None,
        ),
    ];
    let drying = match settings.wetness {
        Wetness::On { drying } => Some(drying),
        Wetness::Off => None,
    };
    parts.push(labelled(
        tr!("animate.liquid.wetness"),
        row_of([
            spacer().grow(1.0).any(),
            switch(drying.is_some())
                .control_size(ControlSize::Small)
                .on_toggle(desk.act_with(move |studio, on: bool| {
                    let mut settings = studio.fluid_settings();
                    settings.wetness = match on {
                        true => Wetness::default(),
                        false => Wetness::Off,
                    };
                    studio.set_fluid_settings(settings);
                }))
                .any(),
        ])
        .align_items(Align::Center)
        .fill_width()
        .any(),
        None,
    ));
    if let Some(drying) = drying {
        parts.push(labelled(
            tr!("animate.liquid.drying"),
            tuned(
                desk.reach(),
                drying,
                FluidSettings::DRYING_SECONDS,
                1.0,
                |settings, drying| settings.wetness = Wetness::On { drying },
            ),
            None,
        ));
    }
    framed(parts)
}

fn pulled(pull: f32) -> Gravity {
    match (pull - Gravity::EARTH).abs() < 0.01 {
        true => Gravity::Earth,
        false => Gravity::Custom(Vec3::new(0.0, -pull, 0.0)),
    }
}

fn tuned(
    reach: Reach,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    step: f32,
    change: fn(&mut FluidSettings, f32),
) -> Element {
    let control = slider(value.clamp(*range.start(), *range.end())).range(range);
    let control = match step > 0.0 {
        true => control.step(step),
        false => control,
    };
    control
        .control_size(ControlSize::Small)
        .on_change(reach.act_with(move |studio, value: f32| {
            let mut settings = studio.fluid_settings();
            change(&mut settings, value);
            studio.draft_fluid_settings(settings);
        }))
        .on_commit(reach.act_with(move |studio, value: f32| {
            let mut settings = studio.fluid_settings();
            change(&mut settings, value);
            studio.set_fluid_settings(settings);
        }))
        .fill_width()
        .any()
}
