use animate_export::{
    CODECS, CONTAINERS, ENCODERS, Encoding, FRAME_RATES, Plan, QUALITIES, RATES, Rate,
    SOUND_BITRATES, SPEEDS, position,
};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::size::{self, Purpose};
use super::{STAGE, labelled};
use crate::animate::Animate;
use crate::output::{Exporting, frame};

const WIDTH: f32 = 720.0;
const FRAME_DIGITS: usize = 6;
const BITRATE_DIGITS: usize = 3;
const READOUT_WIDTH: f32 = 32.0;

pub(super) fn sheet(exporting: &Exporting, last_frame: u32) -> Element {
    let body = row! {
        container(picture(exporting, last_frame)).width(0.0).grow(1.0);
        container(encoding(exporting)).width(0.0).grow(1.0);
    }
    .gap(Space::LOOSE)
    .align_items(Align::Start)
    .fill_width();
    dialog(tr!("animate.export.title"), body)
        .cancel(tr!("animate.output.cancel"), |animate: &mut Animate| {
            animate.output_mut().close();
        })
        .accept(tr!("animate.export.export"), Animate::export)
        .ready(exporting.export().is_some() && !exporting.is_asking())
        .width(WIDTH)
        .keyed(("animate-export-sheet", exporting.key()))
        .centered_on(STAGE)
        .build()
}

fn tune(animate: &mut Animate, change: impl FnOnce(&mut Encoding)) {
    if let Some(exporting) = animate.output_mut().exporting_mut() {
        change(exporting.encoding_mut());
    }
}

fn heading(caption: Label) -> impl View {
    text(caption).caption().semibold()
}

fn picture(exporting: &Exporting, last_frame: u32) -> impl View {
    let encoding = exporting.encoding();
    let rates: Vec<Label> = FRAME_RATES
        .iter()
        .map(|(_, name)| Label::from(format!("{name} fps")))
        .collect();
    column! {
        heading(tr!("animate.export.picture"));
        size::rows(exporting.sizing(), Purpose::Export);
        labelled(
            tr!("animate.export.frame_rate"),
            combo(position(&FRAME_RATES, encoding.frame_rate))
                .options(rates)
                .on_select(|animate: &mut Animate, chosen: usize| {
                    tune(animate, |encoding| encoding.frame_rate = FRAME_RATES[chosen].0);
                })
                .grow(1.0),
        );
        labelled(tr!("animate.export.range"), range(exporting, last_frame));
        labelled(tr!("animate.export.length"), length(exporting));
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn range(exporting: &Exporting, last_frame: u32) -> impl View {
    let valid_range = exporting.range().is_some();
    row! {
        frame_field(exporting.first(), valid_range).grow(1.0);
        text("–").muted();
        frame_field(exporting.last(), valid_range).grow(1.0);
        button(icon(MaterialSymbol::SELECT_ALL).extent(Length::px(18.0)))
            .icon_only()
            .small()
            .ghost()
            .on_press(move |animate: &mut Animate| {
                if let Some(exporting) = animate.output_mut().exporting_mut() {
                    exporting.first().set_text("0");
                    exporting.last().set_text(last_frame.to_string());
                }
            })
            .tooltip(tr!("animate.tip.all_frames"));
    }
    .gap(Space::TIGHT)
    .align_items(Align::Center)
    .grow(1.0)
}

fn frame_field(typed: &TextDocument, ordered: bool) -> TextField {
    text_field(typed)
        .max_len(FRAME_DIGITS)
        .error(!ordered || frame(&typed.text()).is_none())
        .on_submit(|animate: &mut Animate| animate.export())
}

fn length(exporting: &Exporting) -> impl View {
    let label = exporting.range().map_or_else(
        || Label::from("—"),
        |(first, last)| {
            let plan = Plan::new(first, last, exporting.encoding().frame_rate);
            tr!(
                "animate.export.frames",
                duration = rikka_format::format_duration(plan.duration()).as_str(),
                frames = plan.frames()
            )
        },
    );
    text(label).caption().grow(1.0)
}

fn encoding(exporting: &Exporting) -> impl View {
    let encoding = exporting.encoding();
    let amount = match encoding.rate {
        Rate::Quality => labelled(tr!("animate.export.quality"), quality(encoding.quality)).build(),
        Rate::Bitrate => labelled(tr!("animate.export.bitrate"), bitrate(exporting)).build(),
    };
    column! {
        heading(tr!("animate.export.encoding"));
        labelled(
            tr!("animate.export.format"),
            choices(&CONTAINERS, encoding.container, |container| animate_export::container_label(*container))
                .on_select(|animate: &mut Animate, chosen: usize| {
                    tune(animate, |encoding| encoding.container = CONTAINERS[chosen].0);
                })
                .grow(1.0),
        );
        labelled(
            tr!("animate.export.codec"),
            segmented(position(&CODECS, encoding.codec).unwrap_or(0))
                .segments(CODECS.iter().map(|(codec, _)| animate_export::codec_label(*codec)))
                .control_size(ControlSize::Small)
                .on_select(|animate: &mut Animate, chosen: usize| {
                    tune(animate, |encoding| encoding.codec = CODECS[chosen].0);
                })
                .grow(1.0),
        );
        labelled(
            tr!("animate.export.encoder"),
            choices(&ENCODERS, encoding.encoder, |encoder| animate_export::encoder_label(*encoder))
                .on_select(|animate: &mut Animate, chosen: usize| {
                    tune(animate, |encoding| encoding.encoder = ENCODERS[chosen].0);
                })
                .grow(1.0),
        );
        labelled(
            tr!("animate.export.speed"),
            choices(&SPEEDS, encoding.speed, |speed| animate_export::speed_label(*speed))
                .on_select(|animate: &mut Animate, chosen: usize| {
                    tune(animate, |encoding| encoding.speed = SPEEDS[chosen].0);
                })
                .grow(1.0),
        );
        labelled(
            tr!("animate.export.rate"),
            segmented(position(&RATES, encoding.rate).unwrap_or(0))
                .segments(RATES.iter().map(|(rate, _)| animate_export::rate_label(*rate)))
                .control_size(ControlSize::Small)
                .on_select(|animate: &mut Animate, chosen: usize| {
                    tune(animate, |encoding| encoding.rate = RATES[chosen].0);
                })
                .grow(1.0),
        );
        amount;
        labelled(tr!("animate.export.sound"), sound(exporting));
    }
    .gap(Space::ROOMY)
    .fill_width()
}

fn choices<T: Copy + PartialEq>(
    table: &[(T, &str)],
    chosen: T,
    label: impl Fn(&T) -> Label,
) -> Combo {
    combo(position(table, chosen)).options(table.iter().map(|(value, _)| label(value)))
}

fn quality(level: u8) -> impl View {
    row! {
        slider(f32::from(level))
            .range(f32::from(*QUALITIES.start())..=f32::from(*QUALITIES.end()))
            .step(1.0)
            .control_size(ControlSize::Small)
            .on_change(|animate: &mut Animate, level: f32| {
                tune(animate, |encoding| encoding.quality = level.round() as u8);
            })
            .grow(1.0);
        text(level.to_string()).caption().width(READOUT_WIDTH);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .grow(1.0)
}

fn bitrate(exporting: &Exporting) -> impl View {
    row! {
        text_field(exporting.bitrate())
            .max_len(BITRATE_DIGITS)
            .error(exporting.typed_bitrate().is_none())
            .on_submit(|animate: &mut Animate| animate.export())
            .grow(1.0);
        text("Mbit/s").caption().muted();
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .grow(1.0)
}

fn sound(exporting: &Exporting) -> impl View {
    let encoding = exporting.encoding();
    let has_sound = exporting.has_sound();
    row! {
        switch(has_sound && encoding.sound)
            .control_size(ControlSize::Small)
            .enabled(has_sound)
            .on_toggle(|animate: &mut Animate, on: bool| tune(animate, |encoding| encoding.sound = on));
        combo(SOUND_BITRATES.iter().position(|bitrate| *bitrate == encoding.sound_bitrate))
        .options(SOUND_BITRATES.map(|bitrate| format!("AAC {bitrate} kbit/s")))
        .enabled(has_sound && encoding.sound)
        .on_select(|animate: &mut Animate, chosen: usize| {
            tune(animate, |encoding| encoding.sound_bitrate = SOUND_BITRATES[chosen]);
        })
        .grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .grow(1.0)
}
