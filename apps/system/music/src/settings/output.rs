use music_preferences::{GAIN_MODES, PREAMP_DB};
use rikka_app::prelude::*;
use rikka_hifi::{Device, Output, OutputState, ReplayGain, ReplayGainMode};
use rikka_widgets::{column, row};

use crate::music::Music;
use crate::signal::refusal;
use crate::wording::{dsd, hertz};

const CHOICE: f32 = 56.0;
const MODE_WIDTH: f32 = 260.0;
const PREAMP_STEP: f32 = 0.5;
const PREAMP_WIDTH: f32 = 200.0;
const READOUT: f32 = 64.0;

struct Choice {
    symbol: MaterialSymbol,
    title: String,
    detail: String,
    output: Output,
    error_message: Option<String>,
}

pub(super) fn view(music: &Music) -> impl View {
    let sound = music.sound();
    let chosen = sound.output().clone();
    let state = sound.state().cloned();
    let mut choices = vec![Choice {
        symbol: MaterialSymbol::SPEAKER,
        title: tr!("music.output.system").to_string(),
        detail: tr!("music.output.shared").to_string(),
        output: Output::Shared,
        error_message: None,
    }];
    choices.extend(sound.sinks().iter().map(|sink| Choice {
        symbol: MaterialSymbol::SPEAKER,
        title: sink.title.clone(),
        detail: tr!("music.output.shared").to_string(),
        output: Output::Sink(sink.name.clone()),
        error_message: None,
    }));
    if let Output::Sink(name) = &chosen
        && !sound.sinks().iter().any(|sink| sink.name == *name)
    {
        choices.push(Choice {
            symbol: MaterialSymbol::SPEAKER,
            title: name.clone(),
            detail: tr!("music.refusal.missing").to_string(),
            output: chosen.clone(),
            error_message: None,
        });
    }
    let shared = choices.len();
    choices.extend(sound.devices().iter().map(|device| Choice {
        symbol: MaterialSymbol::VERIFIED,
        title: device.name.clone(),
        detail: capabilities(device),
        output: Output::Exclusive(device.id.clone()),
        error_message: match &state {
            Some(OutputState::FellBack {
                device: refused_device,
                reason,
            }) if *refused_device == device.id => Some(refusal(reason).to_string()),
            _ => None,
        },
    }));
    let exclusive = choices.split_off(shared);
    let gain = sound.replay_gain();
    column! {
        section(
            tr!("music.output.device"),
            card(
                column! {
                    column_of(choices.into_iter().map(|choice| choice_row(choice, &chosen, &state)))
                        .gap(Space::HAIR)
                        .fill_width();
                    if !exclusive.is_empty() {
                        text(tr!("music.output.exclusive")).caption().muted().semibold();
                        column_of(exclusive.into_iter().map(|choice| choice_row(choice, &chosen, &state)))
                            .gap(Space::HAIR)
                            .fill_width();
                    }
                }
                .gap(Space::SNUG)
                .fill_width(),
            ),
        );
        section(tr!("music.output.levelling"), card(levelling(gain)));
    }
    .gap(Space::NORMAL)
    .fill_width()
}

fn choice_row(choice: Choice, chosen: &Output, state: &Option<OutputState>) -> Element {
    let selected = choice.output == *chosen;
    let playing = matches!(
        (state, &choice.output),
        (Some(OutputState::Exclusive(active)), Output::Exclusive(device)) if active == device
    );
    let output = choice.output;
    let detail = match &choice.error_message {
        Some(error_message) => text(error_message.clone())
            .caption()
            .color(palette().warning)
            .no_wrap(),
        None => text(choice.detail).caption().muted().no_wrap(),
    };
    container(
        row! {
            icon(choice.symbol).extent(Length::px(22.0)).color(match selected {
                true => palette().accent,
                false => palette().text_muted,
            });
            column! {
                text(choice.title).label().medium().no_wrap();
                detail;
            }
            .gap(Space::HAIR)
            .grow(1.0)
            .min_width(0.0);
            if playing {
                text(tr!("music.output.playing")).caption().semibold().color(palette().accent).no_wrap();
            }
            if selected {
                icon(MaterialSymbol::CHECK_CIRCLE)
                    .filled()
                    .extent(Length::px(20.0))
                    .color(palette().accent);
            }
        }
        .gap(Space::NORMAL)
        .align_items(Align::Center)
        .fill_width(),
    )
    .padding(Inset::xy(Space::NORMAL, Space::TIGHT))
    .radius(Corner::Panel)
    .background(match selected {
        true => palette().accent.with_alpha(0.12),
        false => Color::TRANSPARENT,
    })
    .on_press(move |music: &mut Music| music.set_output(output.clone()))
    .height(CHOICE)
    .fill_width()
    .any()
}

fn capabilities(device: &Device) -> String {
    let mut message = Vec::new();
    if let Some(capabilities) = &device.capabilities {
        let bits = capabilities
            .formats
            .iter()
            .filter(|format| format.is_integer())
            .map(|format| format.precision())
            .max();
        let rate = capabilities.rates.iter().copied().max();
        if let (Some(bits), Some(rate)) = (bits, rate) {
            message.push(
                tr!(
                    "music.output.up-to",
                    bits = &bits.to_string(),
                    rate = hertz(rate).as_str()
                )
                .to_string(),
            );
        }
    }
    let dsd_support = device.dsd();
    if let Some(native) = dsd_support.native.iter().copied().max() {
        message.push(tr!("music.output.native", dsd = dsd(native).as_str()).to_string());
    }
    if let Some(dop) = dsd_support.dop.iter().copied().max() {
        message.push(tr!("music.output.dop", dsd = dsd(dop).as_str()).to_string());
    }
    match message.is_empty() {
        true => device.pcm.clone(),
        false => message.join(" · "),
    }
}

fn levelling(gain: ReplayGain) -> impl View {
    let active = gain.mode != ReplayGainMode::Off;
    column! {
        field(
            tr!("music.output.replay-gain"),
            segmented(GAIN_MODES.iter().position(|mode| *mode == gain.mode).unwrap_or_default())
                .segments(GAIN_MODES.map(named))
                .on_select(move |music: &mut Music, index: usize| {
                    if let Some(mode) = GAIN_MODES.get(index) {
                        music.set_replay_gain(ReplayGain { mode: *mode, ..gain });
                    }
                })
                .width(MODE_WIDTH),
        );
        field(
            tr!("music.output.preamp"),
            row! {
                slider(gain.preamp_db)
                    .range(-PREAMP_DB..=PREAMP_DB)
                    .step(PREAMP_STEP)
                    .enabled(active)
                    .on_change(move |music: &mut Music, db: f32| {
                        music.set_replay_gain(ReplayGain { preamp_db: db, ..gain });
                    })
                    .width(PREAMP_WIDTH);
                text(tr!("music.signal.decibels", db = &format!("{:+.1}", gain.preamp_db)))
                    .label()
                    .muted()
                    .align(TextAlign::End)
                    .width(READOUT);
            }
            .gap(Space::SNUG)
            .align_items(Align::Center),
        )
        .dimmed(!active);
        field(
            tr!("music.output.clipping"),
            switch(gain.prevent_clipping)
                .enabled(active)
                .on_toggle(move |music: &mut Music, prevent: bool| {
                    music.set_replay_gain(ReplayGain { prevent_clipping: prevent, ..gain });
                }),
        )
        .dimmed(!active);
    }
    .gap(Space::SNUG)
    .fill_width()
}

fn named(mode: ReplayGainMode) -> Label {
    match mode {
        ReplayGainMode::Off => tr!("music.output.off"),
        ReplayGainMode::Track => tr!("music.output.track"),
        ReplayGainMode::Album => tr!("music.output.album"),
    }
}
