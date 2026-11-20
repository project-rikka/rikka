use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use rikka_sound::config::{DeviceTuning, PERIODS, RESAMPLE_QUALITIES};
use rikka_sound::{Command, Hardware, Node, QUANTUMS, RATES, SampleFormat};

use crate::amount::amount;
use crate::audio::component::{Audio, option_at};
use crate::audio::readout;
use crate::picker::default_picker;

const HEADROOMS: [u32; 7] = [0, 32, 64, 128, 256, 1024, 8192];

const OFFSET: f32 = 500.0;

const CHECKBOXES: f32 = 294.0;

pub(super) fn view(audio: &Audio) -> impl View {
    column! {
        toggle(audio);
        if *audio.advanced {
            section(tr!("settings.audio.clock"), clock(audio));
            if let Some(sink) = audio.graph().sink() {
                section(tr!("settings.audio.hardware"), hardware(audio, sink));
            }
        }
    }
    .gap(Space::ROOMY)
}

fn toggle(audio: &Audio) -> impl View {
    let showing = *audio.advanced;
    row! {
        button(
            row! {
                icon(if showing {
                    MaterialSymbol::EXPAND_LESS
                } else {
                    MaterialSymbol::EXPAND_MORE
                })
                .extent(Length::px(16.0))
                .color(palette().text_muted);
                text(tr!("settings.audio.advanced")).caption().muted();
            }
            .gap(Space::TIGHT)
            .align_items(Align::Center),
        )
        .ghost()
        .small()
        .on_press(move |audio: &mut Audio| audio.advanced.set(!showing));
        spacer();
    }
    .align_items(Align::Center)
}

fn clock(audio: &Audio) -> impl View {
    let graph = &audio.graph().graph;
    let tuning = &audio.tuning;
    let running = tr!(
        "settings.audio.running",
        rate = &readout::hertz(graph.running_rate()),
        quantum = &graph.running_quantum().to_string(),
        latency = &graph
            .latency()
            .map_or_else(|| "\u{2014}".to_owned(), readout::milliseconds),
    );

    column! {
        field(tr!("settings.audio.now"), text(running).label().muted());
        automatic(
            tr!("settings.audio.rate"),
            &RATES,
            tuning.rate,
            readout::hertz,
            |audio: &mut Audio, index: Option<usize>| {
                audio.tuning.rate = option_at(&RATES, index);
                audio.send(Command::ForceRate(audio.tuning.rate.unwrap_or(0)));
                audio.pin_graph();
            },
        );
        automatic(
            tr!("settings.audio.quantum"),
            &QUANTUMS,
            tuning.quantum,
            samples,
            |audio: &mut Audio, index: Option<usize>| {
                audio.tuning.quantum = option_at(&QUANTUMS, index);
                audio.send(Command::ForceQuantum(audio.tuning.quantum.unwrap_or(0)));
                audio.pin_graph();
            },
        );
        automatic(
            tr!("settings.audio.min-quantum"),
            &QUANTUMS,
            tuning.min_quantum,
            samples,
            |audio: &mut Audio, index: Option<usize>| {
                audio.tuning.min_quantum = option_at(&QUANTUMS, index);
                audio.pin_graph();
            },
        );
        automatic(
            tr!("settings.audio.max-quantum"),
            &QUANTUMS,
            tuning.max_quantum,
            samples,
            |audio: &mut Audio, index: Option<usize>| {
                audio.tuning.max_quantum = option_at(&QUANTUMS, index);
                audio.pin_graph();
            },
        );
        field(tr!("settings.audio.allowed-rates"), allowed(audio));
        resampler(audio);
    }
    .gap(Space::SNUG)
}

fn allowed(audio: &Audio) -> impl View {
    row! {
        for rate in RATES {
            checkbox(audio.tuning.allowed_rates.contains(&rate))
                .label(text(readout::hertz(rate)).caption())
                .on_toggle(move |audio: &mut Audio, allowed: bool| {
                    audio.tuning.allowed_rates.retain(|allowed| *allowed != rate);
                    if allowed {
                        audio.tuning.allowed_rates.push(rate);
                        audio.tuning.allowed_rates.sort_unstable();
                    }
                    audio.pin_graph();
                });
        }
    }
    .gap(Space::NORMAL)
    .wrap_items()
    .width(CHECKBOXES)
}

fn resampler(audio: &Audio) -> impl View {
    let quality = audio.tuning.resample_quality.unwrap_or(4);
    field(
        tr!("settings.audio.resample-quality"),
        amount(
            quality as f32,
            *RESAMPLE_QUALITIES.start() as f32..=*RESAMPLE_QUALITIES.end() as f32,
            1.0,
            quality.to_string(),
            |audio: &mut Audio, quality: f32| {
                audio.tuning.resample_quality = Some(quality.round() as u32);
                audio.pin_graph();
            },
        ),
    )
}

fn hardware(audio: &Audio, sink: &Node) -> impl View {
    let tuning = audio.tunings.get(&sink.name).cloned().unwrap_or_default();
    let volume = audio.volume_at(sink.id);

    column! {
        text(sink.name.as_str()).caption().muted();
        field(
            tr!("settings.audio.exact-volume"),
            text(format!(
                "{} \u{b7} {:.4}",
                readout::decibels(&volume),
                volume.amplitude(),
            ))
            .label()
            .muted(),
        );
        if let Some(card) = &sink.hardware {
            card_settings(card, &tuning);
        }
        offset(sink);
    }
    .gap(Space::SNUG)
}

fn card_settings(card: &Hardware, tuning: &DeviceTuning) -> impl View {
    column! {
        pinned(
            tr!("settings.audio.sample-format"),
            formats(card, tuning.format),
            tuning.format,
            depth,
            |tuning, format| tuning.format = format,
        );
        pinned(
            tr!("settings.audio.rate"),
            rates(card, tuning.rate),
            tuning.rate,
            readout::hertz,
            |tuning, rate| tuning.rate = rate,
        );
        automatic(
            tr!("settings.audio.period"),
            &PERIODS,
            tuning.period,
            samples,
            |audio: &mut Audio, index: Option<usize>| {
                audio.tune(|tuning| tuning.period = option_at(&PERIODS, index));
            },
        );
        automatic(
            tr!("settings.audio.headroom"),
            &HEADROOMS,
            tuning.headroom,
            samples,
            |audio: &mut Audio, index: Option<usize>| {
                audio.tune(|tuning| tuning.headroom = option_at(&HEADROOMS, index));
            },
        );
        field(
            tr!("settings.audio.disable-batch"),
            switch(tuning.disable_batch).on_toggle(|audio: &mut Audio, disabled: bool| {
                audio.tune(|tuning| tuning.disable_batch = disabled);
            }),
        );
    }
    .gap(Space::SNUG)
}

fn formats(card: &Hardware, pinned: Option<SampleFormat>) -> Vec<(SampleFormat, bool)> {
    SampleFormat::ALL
        .into_iter()
        .filter_map(|format| {
            let is_pinned = pinned == Some(format);
            let error = is_pinned && card.refuses_pinned_format;
            let unknown = is_pinned && card.formats.is_empty();
            let playable = !error && (card.plays(format) || unknown);
            (card.plays(format) || is_pinned).then_some((format, playable))
        })
        .collect()
}

fn rates(card: &Hardware, pinned: Option<u32>) -> Vec<(u32, bool)> {
    RATES
        .into_iter()
        .filter_map(|rate| {
            let is_pinned = pinned == Some(rate);
            let playable = card.takes(rate) || (is_pinned && card.rates.is_none());
            (card.takes(rate) || is_pinned).then_some((rate, playable))
        })
        .collect()
}

fn pinned<T: Copy + PartialEq + 'static>(
    label: Label,
    offered: Vec<(T, bool)>,
    selected: Option<T>,
    show: fn(T) -> String,
    pin: fn(&mut DeviceTuning, Option<T>),
) -> impl View {
    let selected =
        selected.and_then(|selected| offered.iter().position(|(value, _)| *value == selected));
    let labels: Vec<Label> = offered
        .iter()
        .map(|(value, playable)| {
            if *playable {
                Label::from(show(*value))
            } else {
                tr!("settings.audio.unsupported", value = &show(*value))
            }
        })
        .collect();
    let values: Vec<T> = offered.into_iter().map(|(value, _)| value).collect();
    field(
        label,
        default_picker(
            tr!("settings.audio.automatic"),
            labels,
            selected,
            move |audio: &mut Audio, index: Option<usize>| {
                let value = index.and_then(|index| values.get(index).copied());
                audio.tune(|tuning| pin(tuning, value));
            },
        ),
    )
}

fn offset(sink: &Node) -> impl View {
    let milliseconds = sink.latency_offset as f64 / 1_000_000.0;
    field(
        tr!("settings.audio.latency-offset"),
        amount(
            milliseconds as f32,
            -OFFSET..=OFFSET,
            1.0,
            readout::milliseconds(milliseconds),
            |audio: &mut Audio, milliseconds: f32| {
                if let Some(node) = audio.graph().sink().map(|sink| sink.id) {
                    audio.send(Command::LatencyOffset {
                        node,
                        nanoseconds: (f64::from(milliseconds) * 1_000_000.0) as i64,
                    });
                }
            },
        ),
    )
}

fn automatic<T: Copy + PartialEq + 'static, M>(
    label: Label,
    offered: &'static [T],
    selected: Option<T>,
    show: fn(T) -> String,
    choose: impl IntoHandler<(Option<usize>,), (), M>,
) -> impl View {
    field(
        label,
        default_picker(
            tr!("settings.audio.automatic"),
            offered.iter().map(|value| Label::from(show(*value))),
            selected.and_then(|value| offered.iter().position(|offered| *offered == value)),
            choose,
        ),
    )
}

fn samples(count: u32) -> String {
    format!("{count}")
}

fn depth(format: SampleFormat) -> String {
    let kind = if format.floating() {
        "float"
    } else {
        "integer"
    };
    format!("{} ({} bit {kind})", format.name(), format.depth())
}
