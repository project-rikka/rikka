mod words;

use music_core::Fidelity;
use rikka_app::prelude::*;
use rikka_hifi::{OutputPath, OutputState, SharedPath, SignalPath};
use rikka_sound::RATES;
use rikka_widgets::{column, row};

use crate::music::Music;
use crate::widgets::fidelity_label;
use crate::wording::{self, hertz};

pub(crate) use words::refusal;

pub(crate) const PLAYER: &str = "music-signal-player";

const WIDTH: f32 = 360.0;
const DOT: f32 = 10.0;
const RAIL: f32 = 2.0;
const STEP: f32 = 44.0;
const LIFT: f32 = 8.0;

struct Step {
    tone: Color,
    title: String,
    detail: String,
}

pub(crate) fn badge(music: &Music, mark: &'static str) -> Option<Element> {
    let fidelity = music
        .sound()
        .fidelity()
        .filter(|_| !music.player().loading())?;
    Some(
        container(fidelity_label(fidelity))
            .padding(Inset::xy(Space::NONE, Space::HAIR))
            .radius(Corner::Item)
            .on_press(|music: &mut Music| music.open_signal())
            .min_width(0.0)
            .build()
            .keyed(mark),
    )
}

pub(crate) fn view(music: &Music, mark: &'static str) -> Option<Element> {
    if !music.signal_open() {
        return None;
    }
    let path = music.sound().path()?;
    let steps = steps(path);
    let last = steps.len().saturating_sub(1);
    let fallen = match music.sound().state() {
        Some(OutputState::FellBack { device, reason }) => {
            let name = music
                .sound()
                .devices()
                .iter()
                .find(|known| known.id == *device)
                .map_or_else(|| device.pcm(), |known| known.name.clone());
            let reason = refusal(reason).to_string();
            Some(tr!(
                "music.signal.fell-back",
                device = name.as_str(),
                reason = reason.as_str()
            ))
        }
        _ => None,
    };
    let offer = offer(path);
    Some(
        popover(
            column! {
                column_of(steps.into_iter().enumerate().map(|(at, step)| step_line(step, at == last)));
                fallen.map(fallen_note);
                offer.map(|(running, rates)| switching(running, rates));
            }
            .gap(Space::NORMAL)
            .padding(Inset::all(Space::SNUG))
            .max_width(WIDTH),
        )
        .on_dismiss(|music: &mut Music| music.close_signal())
        .build()
        .anchored(Anchor::above(mark).offset(0.0, LIFT)),
    )
}

fn steps(path: &SignalPath) -> Vec<Step> {
    let source = Fidelity::from_source_format(&path.source);
    let bits = path.source.pcm_bits();
    let mut steps = vec![Step {
        tone: match path.source.is_lossy() {
            true => palette().warning,
            false => palette().accent,
        },
        title: tr!("music.signal.source").to_string(),
        detail: format!(
            "{} · {}",
            wording::fidelity(source),
            words::channels(path.source.channels)
        ),
    }];
    steps.extend(path.stages.iter().map(|stage| {
        let (title, detail) = words::stage(stage);
        Step {
            tone: match (stage.reduces_resolution(bits), stage.modifies_samples()) {
                (true, _) => palette().warning,
                (false, true) => palette().success,
                (false, false) => palette().accent,
            },
            title: title.to_string(),
            detail,
        }
    }));
    steps.push(output(&path.output, bits));
    steps
}

fn output(output: &OutputPath, bits: Option<u32>) -> Step {
    let keeps = |precision: u32, dsd: bool| dsd || bits.is_none_or(|bits| precision >= bits);
    match output {
        OutputPath::Shared(shared) => shared_output(shared, keeps),
        OutputPath::Exclusive(exclusive) => {
            let mut detail = vec![
                tr!("music.signal.exclusive").to_string(),
                words::sample(exclusive.format),
            ];
            if !exclusive.format.is_dsd() {
                detail.insert(1, hertz(exclusive.rate));
            }
            if exclusive.dop {
                detail.push(tr!("music.signal.dop").to_string());
            }
            Step {
                tone: match keeps(exclusive.format.precision(), exclusive.format.is_dsd()) {
                    true => palette().accent,
                    false => palette().warning,
                },
                title: exclusive.name.clone(),
                detail: detail.join(" · "),
            }
        }
    }
}

fn shared_output(shared: &SharedPath, keeps: impl Fn(u32, bool) -> bool) -> Step {
    let sink = shared.sink.as_ref();
    let mut detail = vec![String::from("PipeWire")];
    if let Some(rate) = shared.graph_rate() {
        detail.push(hertz(rate));
    }
    if let Some(format) = sink.and_then(|sink| sink.format) {
        detail.push(words::sample(format));
    }
    if let Some(codec) = sink.and_then(|sink| sink.bluetooth.as_ref()) {
        detail.push(tr!("music.signal.bluetooth", codec = codec.as_str()).to_string());
    }
    let lossy = sink.is_some_and(|sink| {
        sink.bluetooth.is_some()
            || sink
                .format
                .is_some_and(|format| !keeps(format.precision(), format.is_dsd()))
    });
    Step {
        tone: match lossy {
            true => palette().warning,
            false => palette().accent,
        },
        title: sink.map_or_else(
            || tr!("music.output.system").to_string(),
            |sink| match sink.description.is_empty() {
                true => sink.name.clone(),
                false => sink.description.clone(),
            },
        ),
        detail: detail.join(" · "),
    }
}

fn offer(path: &SignalPath) -> Option<(u32, Vec<u32>)> {
    let OutputPath::Shared(shared) = &path.output else {
        return None;
    };
    let sink = shared.sink.as_ref()?;
    let running = shared.graph_rate()?;
    let stream_rate = shared.stream_rate;
    let pinned = shared.forced_rate.is_some_and(|rate| rate > 0);
    if running == stream_rate
        || pinned
        || sink.bluetooth.is_some()
        || !sink.hardware_rates.contains(&stream_rate)
        || shared.allowed_rates.contains(&stream_rate)
    {
        return None;
    }
    let rates: Vec<u32> = RATES
        .into_iter()
        .filter(|rate| sink.hardware_rates.contains(rate))
        .collect();
    Some((running, rates))
}

fn switching(running: u32, rates: Vec<u32>) -> Element {
    container(
        column! {
            text(tr!("music.signal.pinned", rate = hertz(running).as_str()))
                .caption()
                .fill_width();
            button(text(tr!("music.signal.allow")).label().semibold())
                .primary()
                .small()
                .on_press(move |music: &mut Music| music.allow_rate_switching(rates.clone()));
        }
        .gap(Space::SNUG)
        .fill_width(),
    )
    .padding(Inset::all(Space::SNUG))
    .background(palette().surface_variant)
    .radius(Corner::Item)
    .fill_width()
    .any()
}

fn fallen_note(message: Label) -> Element {
    row! {
        icon(MaterialSymbol::WARNING).extent(Length::px(16.0)).color(palette().warning);
        column! {
            text(message).caption().semibold();
            text(tr!("music.signal.instead")).caption().muted();
        }
        .gap(Space::HAIR)
        .grow(1.0)
        .min_width(0.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Start)
    .fill_width()
    .any()
}

fn step_line(step: Step, last: bool) -> Element {
    row! {
        column! {
            spacer().height(Length::px((STEP - DOT) / 2.0 - 6.0));
            dot(step.tone);
            if !last {
                container(spacer())
                    .background(palette().outline)
                    .width(RAIL)
                    .grow(1.0);
            }
        }
        .align_items(Align::Center)
        .width(DOT)
        .fill_height();
        column! {
            text(step.title).label().medium().no_wrap();
            if !step.detail.is_empty() {
                text(step.detail).caption().muted();
            }
        }
        .gap(Space::HAIR)
        .padding(Inset::edges(Space::NONE, Space::NONE, Space::SNUG, Space::NONE))
        .grow(1.0)
        .min_width(0.0);
    }
    .gap(Space::NORMAL)
    .any()
}

fn dot(tint: Color) -> Element {
    container(spacer())
        .background(tint)
        .radius(DOT / 2.0)
        .size(Length::px(DOT), Length::px(DOT))
        .any()
}
