use rikka_app::prelude::*;
use rikka_hifi::{Processor, Refusal, SampleFormat, Stage, VolumeControl};

use crate::wording::{dsd, hertz};

const PERCENT: f32 = 100.0;

pub(super) fn stage(stage: &Stage) -> (Label, String) {
    match *stage {
        Stage::DsdToPcm { from, to } => (
            tr!("music.signal.dsd-to-pcm"),
            format!("{} → {}", dsd(from), hertz(to)),
        ),
        Stage::Resample { from, to, by } => (
            tr!("music.signal.resample"),
            format!("{} → {} · {}", hertz(from), hertz(to), processor(by)),
        ),
        Stage::Remix { from, to, by } => (
            match to < from {
                true => tr!("music.signal.downmix"),
                false => tr!("music.signal.upmix"),
            },
            format!("{} → {} · {}", channels(from), channels(to), processor(by)),
        ),
        Stage::ReplayGain { db } => (
            tr!("music.signal.replay-gain"),
            tr!("music.signal.decibels", db = &format!("{db:+.1}")).to_string(),
        ),
        Stage::Volume { level, by } => (
            tr!("music.signal.volume"),
            format!("{}% · {}", (level * PERCENT).round(), control(by)),
        ),
        Stage::Dither { bits } => (
            tr!("music.signal.dither"),
            tr!("music.signal.bits", bits = &bits.to_string()).to_string(),
        ),
        Stage::Dop => (tr!("music.signal.dop"), String::new()),
    }
}

pub(super) fn channels(count: u16) -> String {
    match count {
        1 => tr!("music.signal.mono").to_string(),
        2 => tr!("music.signal.stereo").to_string(),
        count => tr!("music.signal.channels", count = &count.to_string()).to_string(),
    }
}

pub(super) fn sample(format: SampleFormat) -> String {
    match format {
        SampleFormat::S16 => bits(16),
        SampleFormat::S24Packed | SampleFormat::S24 => bits(24),
        SampleFormat::S32 => bits(32),
        SampleFormat::F32 => tr!("music.signal.float", bits = "32").to_string(),
        SampleFormat::F64 => tr!("music.signal.float", bits = "64").to_string(),
        SampleFormat::Dsd8
        | SampleFormat::Dsd16Le
        | SampleFormat::Dsd16Be
        | SampleFormat::Dsd32Le
        | SampleFormat::Dsd32Be => tr!("music.signal.native-dsd").to_string(),
    }
}

pub(crate) fn refusal(refusal: &Refusal) -> Label {
    match refusal {
        Refusal::Missing => tr!("music.refusal.missing"),
        Refusal::Busy {
            holder: Some(holder),
        } => tr!("music.refusal.busy-by", holder = holder.as_str()),
        Refusal::Busy { holder: None } => tr!("music.refusal.busy"),
        Refusal::Unsupported => tr!("music.refusal.unsupported"),
        Refusal::Lost => tr!("music.refusal.lost"),
        Refusal::Released => tr!("music.refusal.released"),
        Refusal::Open(why) => tr!("music.refusal.unopenable", reason = why.as_str()),
    }
}

fn bits(count: u32) -> String {
    tr!("music.signal.bits", bits = &count.to_string()).to_string()
}

fn processor(by: Processor) -> &'static str {
    match by {
        Processor::Rikka => "Rikka",
        Processor::PipeWire => "PipeWire",
    }
}

fn control(by: VolumeControl) -> Label {
    match by {
        VolumeControl::Software => tr!("music.signal.software"),
        VolumeControl::PipeWire => tr!("music.signal.stream"),
        VolumeControl::Sink => tr!("music.signal.device"),
        VolumeControl::Hardware => tr!("music.signal.hardware"),
    }
}
