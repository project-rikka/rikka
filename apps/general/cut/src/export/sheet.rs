use cut_export::{Plan, Preset, Settings, SoundCodec, StillFormat, Target};
use cut_project::{Format, Timecode};
use rikka_app::prelude::*;
use rikka_widgets::{column, row};

use super::Exporting;
use super::details;
use super::wording;
use crate::cut::Cut;
use crate::view::STAGE;

const WIDTH: f32 = 540.0;
const GLYPH: f32 = 18.0;

pub(crate) fn sheet(cut: &Cut, exporting: &Exporting) -> Vec<Element> {
    let project = cut.project();
    let format = project.format();
    let settings = *exporting.settings();
    let ready = exporting.is_ready(project);
    let body = column! {
        segmented(Target::ALL.iter().position(|target| *target == settings.target).unwrap_or(0))
            .segments(Target::ALL.map(wording::target))
            .on_select(|cut: &mut Cut, selected: usize| {
                cut.tune_export(|settings| settings.target = Target::ALL[selected]);
            })
            .fill_width();
        flavour(&settings, format);
        text(facts(cut, exporting)).caption().muted().fill_width();
        location(exporting);
        warnings(cut, exporting);
        unfold(exporting.details());
        collapsible(exporting.details().then(|| details::rows(cut, exporting)));
    }
    .gap(Space::ROOMY)
    .fill_width();
    dialog(tr!("cut.export.title"), body)
        .cancel(tr!("cut.export.cancel"), Cut::close_export_sheet)
        .accept(tr!("cut.export.export"), Cut::export)
        .ready(ready)
        .width(WIDTH)
        .keyed(("cut-export", exporting.key()))
        .centered_on(STAGE)
        .with_scrim()
}

fn flavour(settings: &Settings, format: Format) -> Element {
    match settings.target {
        Target::Video => {
            let selected = settings.video.preset(format);
            let mut labels: Vec<Label> = Preset::ALL.map(wording::preset).into();
            if selected.is_none() {
                labels.push(tr!("cut.export.custom"));
            }
            let selected = selected
                .and_then(|preset| {
                    Preset::ALL
                        .iter()
                        .position(|candidate| *candidate == preset)
                })
                .unwrap_or(Preset::ALL.len());
            segmented(selected)
                .segments(labels)
                .on_select(move |cut: &mut Cut, selected: usize| {
                    if let Some(preset) = Preset::ALL.get(selected).copied() {
                        cut.tune_export(|settings| {
                            settings.video = settings.video.with_preset(preset, format);
                        });
                    }
                })
                .fill_width()
                .build()
        }
        Target::Audio => segmented(
            SoundCodec::ALL
                .iter()
                .position(|candidate| *candidate == settings.audio.codec)
                .unwrap_or(0),
        )
        .segments(SoundCodec::ALL.map(wording::sound_codec))
        .on_select(|cut: &mut Cut, selected: usize| {
            cut.tune_export(|settings| settings.audio.codec = SoundCodec::ALL[selected]);
        })
        .fill_width()
        .build(),
        Target::Still => segmented(
            StillFormat::ALL
                .iter()
                .position(|candidate| *candidate == settings.still.format)
                .unwrap_or(0),
        )
        .segments(StillFormat::ALL.map(wording::still_format))
        .on_select(|cut: &mut Cut, selected: usize| {
            cut.tune_export(|settings| settings.still.format = StillFormat::ALL[selected]);
        })
        .fill_width()
        .build(),
    }
}

fn facts(cut: &Cut, exporting: &Exporting) -> String {
    let project = cut.project();
    let format = project.format();
    let settings = exporting.settings();
    let length = |output| {
        exporting.span(project).map_or_else(
            || tr!("cut.export.nothing"),
            |span| {
                let plan = Plan::new(span, format.rate, output);
                Label::from(rikka_format::format_duration(plan.duration()))
            },
        )
    };
    let facts = match settings.target {
        Target::Video => {
            let video = &settings.video;
            let rate = video.frame_rate.rate(format.rate);
            vec![
                Label::from(wording::size(video.resolution.size(format))),
                Label::from(wording::rate(rate)),
                wording::codec(video.codec),
                wording::range(exporting.range()),
                length(rate),
            ]
        }
        Target::Audio => {
            let audio = &settings.audio;
            let quality = match audio.codec {
                SoundCodec::Wav => wording::depth(audio.depth),
                SoundCodec::Aac => Label::from(wording::kilobits(audio.bitrate)),
            };
            vec![
                Label::from(wording::sample_rate(audio.sample_rate)),
                quality,
                wording::range(exporting.range()),
                length(format.rate),
            ]
        }
        Target::Still => vec![
            Label::from(wording::size(settings.still.resolution.size(format))),
            tr!(
                "cut.export.at",
                timecode = Timecode::from_frame(exporting.frame(), format.rate)
                    .to_string()
                    .as_str()
            ),
        ],
    };
    wording::joined(&facts)
}

fn location(exporting: &Exporting) -> impl View {
    let path = exporting.path();
    let name = path
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let folder = exporting.folder().display().to_string();
    row! {
        icon(MaterialSymbol::FOLDER)
            .extent(Length::px(GLYPH))
            .color(palette().text_muted);
        column! {
            text(name).semibold().no_wrap();
            text(folder).caption().muted().no_wrap();
        }
        .gap(Space::NONE)
        .grow(1.0)
        .shrink(1.0);
        button(text(tr!("cut.export.change")))
            .small()
            .enabled(!exporting.is_asking())
            .loading(exporting.is_asking())
            .on_press(Cut::select_export_location);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
}

fn warnings(cut: &Cut, exporting: &Exporting) -> Option<Element> {
    let settings = exporting.settings();
    let missing = cut.offline().len();
    let warning = if let Some(trouble) = exporting.trouble() {
        trouble.clone()
    } else if settings.target != Target::Still && exporting.span(cut.project()).is_none() {
        tr!("cut.export.empty_range")
    } else if settings.target == Target::Audio && !cut.project().is_audible() {
        tr!("cut.export.silent")
    } else if missing > 0 && settings.target != Target::Audio {
        tr!("cut.export.offline", count = missing)
    } else {
        return None;
    };
    Some(
        text(warning)
            .caption()
            .color(palette().danger)
            .fill_width()
            .build(),
    )
}

fn unfold(open: bool) -> impl View {
    container(
        row! {
            icon(match open {
                true => MaterialSymbol::EXPAND_MORE,
                false => MaterialSymbol::CHEVRON_RIGHT,
            })
            .extent(Length::px(GLYPH))
            .color(palette().text_muted);
            text(tr!("cut.export.details")).muted();
        }
        .gap(Space::TIGHT)
        .align_items(Align::Center),
    )
    .on_press(Cut::toggle_export_details)
    .focusless()
    .fill_width()
}
