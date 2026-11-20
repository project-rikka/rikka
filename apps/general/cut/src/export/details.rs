use cut_export::{
    AAC_BITRATES, Audio, Compression, Container, FrameRate, Range, Resolution, Settings,
    SoundCodec, Target, Video,
};
use cut_project::Format;
use rikka_app::prelude::*;
use rikka_widgets::row;

use super::Exporting;
use super::wording;
use crate::cut::Cut;

const CAPTION_WIDTH: f32 = 128.0;
const READOUT_WIDTH: f32 = 32.0;

pub(super) fn rows(cut: &Cut, exporting: &Exporting) -> Element {
    let project = cut.project();
    let format = project.format();
    let settings = exporting.settings();
    let marked = Range::suggested(project) == Range::Marks;
    let mut rows: Vec<Element> = Vec::new();
    if settings.target != Target::Still {
        rows.push(range_row(exporting.range(), marked).build());
    }
    match settings.target {
        Target::Video => rows.extend(video_rows(&settings.video, format)),
        Target::Audio => rows.extend(audio_rows(&settings.audio)),
        Target::Still => rows.push(resolution_row(
            settings.still.resolution,
            format,
            |settings| &mut settings.still.resolution,
        )),
    }
    column_of(rows)
        .gap(Space::SNUG)
        .padding(Inset::xy(Space::NONE, Space::TIGHT))
        .fill_width()
        .build()
}

fn labelled(caption: Label, control: impl View) -> Element {
    row! {
        text(caption).caption().muted().width(CAPTION_WIDTH).shrink(0.0);
        control;
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .fill_width()
    .build()
}

fn position<T: PartialEq>(table: &[T], value: &T) -> Option<usize> {
    table.iter().position(|candidate| candidate == value)
}

fn range_row(range: Range, marked: bool) -> impl View {
    let ranges = [Range::Whole, Range::Marks];
    labelled(
        tr!("cut.export.range"),
        combo(position(&ranges, &range))
            .options(ranges.map(wording::range))
            .enabled(marked || range == Range::Marks)
            .on_select(move |cut: &mut Cut, selected: usize| {
                cut.select_export_range(ranges[selected])
            })
            .grow(1.0),
    )
}

fn resolution_row(
    resolution: Resolution,
    format: Format,
    field: fn(&mut Settings) -> &mut Resolution,
) -> Element {
    let offered: Vec<Resolution> = Resolution::offered(format).collect();
    let labels: Vec<Label> = offered
        .iter()
        .map(|resolution| wording::resolution(*resolution, format))
        .collect();
    labelled(
        tr!("cut.export.resolution"),
        combo(position(&offered, &resolution))
            .options(labels)
            .on_select(move |cut: &mut Cut, selected: usize| {
                let picked = offered[selected];
                cut.tune_export(|settings| *field(settings) = picked);
            })
            .grow(1.0),
    )
}

fn video_rows(video: &Video, format: Format) -> Vec<Element> {
    let rates: Vec<FrameRate> = FrameRate::offered().collect();
    let rate_labels: Vec<Label> = rates
        .iter()
        .map(|rate| wording::frame_rate(*rate, format.rate))
        .collect();
    let amount = match video.compression {
        Compression::Quality => labelled(tr!("cut.export.quality"), quality(video.quality)),
        Compression::Bitrate => labelled(
            tr!("cut.export.bitrate"),
            number_field(video.bitrate as f32)
                .range(*Video::BITRATES.start() as f32..=*Video::BITRATES.end() as f32)
                .step(1.0)
                .decimals(0)
                .unit("Mbit/s")
                .control_size(ControlSize::Small)
                .on_commit(|cut: &mut Cut, bitrate: f32| {
                    cut.tune_export(|settings| settings.video.bitrate = bitrate.round() as u32);
                })
                .grow(1.0),
        ),
    };
    vec![
        resolution_row(video.resolution, format, |settings| {
            &mut settings.video.resolution
        }),
        labelled(
            tr!("cut.export.frame_rate"),
            combo(position(&rates, &video.frame_rate))
                .options(rate_labels)
                .on_select(move |cut: &mut Cut, selected: usize| {
                    let picked = rates[selected];
                    cut.tune_export(|settings| settings.video.frame_rate = picked);
                })
                .grow(1.0),
        ),
        labelled(
            tr!("cut.export.format"),
            combo(position(&Container::ALL, &video.container))
                .options(Container::ALL.map(wording::container))
                .on_select(|cut: &mut Cut, selected: usize| {
                    cut.tune_export(|settings| settings.video.container = Container::ALL[selected]);
                })
                .grow(1.0),
        ),
        labelled(
            tr!("cut.export.codec"),
            segmented(position(&Video::CODECS, &video.codec).unwrap_or(0))
                .segments(Video::CODECS.map(wording::codec))
                .control_size(ControlSize::Small)
                .on_select(|cut: &mut Cut, selected: usize| {
                    cut.tune_export(|settings| settings.video.codec = Video::CODECS[selected]);
                })
                .grow(1.0),
        ),
        labelled(
            tr!("cut.export.encoder"),
            combo(position(&Video::ENCODERS, &video.encoder))
                .options(Video::ENCODERS.map(wording::encoder))
                .on_select(|cut: &mut Cut, selected: usize| {
                    cut.tune_export(|settings| settings.video.encoder = Video::ENCODERS[selected]);
                })
                .grow(1.0),
        ),
        labelled(
            tr!("cut.export.speed"),
            combo(position(&Video::SPEEDS, &video.speed))
                .options(Video::SPEEDS.map(wording::speed))
                .on_select(|cut: &mut Cut, selected: usize| {
                    cut.tune_export(|settings| settings.video.speed = Video::SPEEDS[selected]);
                })
                .grow(1.0),
        ),
        labelled(
            tr!("cut.export.compression"),
            segmented(position(&Compression::ALL, &video.compression).unwrap_or(0))
                .segments(Compression::ALL.map(wording::compression))
                .control_size(ControlSize::Small)
                .on_select(|cut: &mut Cut, selected: usize| {
                    cut.tune_export(|settings| {
                        settings.video.compression = Compression::ALL[selected];
                    });
                })
                .grow(1.0),
        ),
        amount,
        labelled(tr!("cut.export.sound"), sound(video)),
    ]
}

fn quality(level: u8) -> impl View {
    row! {
        slider(f32::from(level))
            .range(f32::from(*Video::QUALITIES.start())..=f32::from(*Video::QUALITIES.end()))
            .step(1.0)
            .control_size(ControlSize::Small)
            .on_change(|cut: &mut Cut, level: f32| {
                cut.tune_export(|settings| settings.video.quality = level.round() as u8);
            })
            .grow(1.0);
        text(level.to_string()).caption().width(READOUT_WIDTH);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .grow(1.0)
}

fn sound(video: &Video) -> impl View {
    row! {
        switch(video.sound)
            .control_size(ControlSize::Small)
            .on_toggle(|cut: &mut Cut, on: bool| {
                cut.tune_export(|settings| settings.video.sound = on);
            });
        combo(position(&AAC_BITRATES, &video.sound_bitrate))
            .options(AAC_BITRATES.map(|bitrate| format!("AAC {}", wording::kilobits(bitrate))))
            .enabled(video.sound)
            .on_select(|cut: &mut Cut, selected: usize| {
                cut.tune_export(|settings| {
                    settings.video.sound_bitrate = AAC_BITRATES[selected];
                });
            })
            .grow(1.0);
    }
    .gap(Space::SNUG)
    .align_items(Align::Center)
    .grow(1.0)
}

fn audio_rows(audio: &Audio) -> Vec<Element> {
    let mut rows = vec![labelled(
        tr!("cut.export.sample_rate"),
        combo(position(&Audio::SAMPLE_RATES, &audio.sample_rate))
            .options(Audio::SAMPLE_RATES.map(wording::sample_rate))
            .on_select(|cut: &mut Cut, selected: usize| {
                cut.tune_export(|settings| {
                    settings.audio.sample_rate = Audio::SAMPLE_RATES[selected]
                });
            })
            .grow(1.0),
    )];
    rows.push(match audio.codec {
        SoundCodec::Wav => labelled(
            tr!("cut.export.depth"),
            segmented(position(&Audio::DEPTHS, &audio.depth).unwrap_or(0))
                .segments(Audio::DEPTHS.map(wording::depth))
                .control_size(ControlSize::Small)
                .on_select(|cut: &mut Cut, selected: usize| {
                    cut.tune_export(|settings| settings.audio.depth = Audio::DEPTHS[selected]);
                })
                .grow(1.0),
        ),
        SoundCodec::Aac => labelled(
            tr!("cut.export.bitrate"),
            combo(position(&AAC_BITRATES, &audio.bitrate))
                .options(AAC_BITRATES.map(wording::kilobits))
                .on_select(|cut: &mut Cut, selected: usize| {
                    cut.tune_export(|settings| settings.audio.bitrate = AAC_BITRATES[selected]);
                })
                .grow(1.0),
        ),
    });
    rows
}
