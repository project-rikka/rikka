use crate::device_id::DeviceId;
use crate::sample_format::SampleFormat;
use crate::source_format::SourceFormat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Processor {
    Rikka,
    PipeWire,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VolumeControl {
    Software,
    PipeWire,
    Sink,
    Hardware,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    DsdToPcm { from: u32, to: u32 },
    Resample { from: u32, to: u32, by: Processor },
    Remix { from: u16, to: u16, by: Processor },
    ReplayGain { db: f32 },
    Volume { level: f32, by: VolumeControl },
    Dither { bits: u32 },
    Dop,
}

impl Stage {
    #[must_use]
    pub fn modifies_samples(&self) -> bool {
        match self {
            Stage::DsdToPcm { .. } | Stage::Resample { .. } | Stage::Dither { .. } => true,
            Stage::Remix { from, to, .. } => to < from,
            Stage::ReplayGain { db } => *db != 0.0,
            Stage::Volume { level, by } => *by != VolumeControl::Hardware && *level != 1.0,
            Stage::Dop => false,
        }
    }

    #[must_use]
    pub fn reduces_resolution(&self, source_bits: Option<u32>) -> bool {
        match self {
            Stage::Resample { from, to, .. } => to < from,
            Stage::Remix { from, to, .. } => to < from,
            Stage::Dither { bits } => source_bits.is_some_and(|source| *bits < source),
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinkPath {
    pub name: String,
    pub description: String,
    pub rate: Option<u32>,
    pub format: Option<SampleFormat>,
    pub channels: Option<u16>,
    pub bluetooth: Option<String>,
    pub card: Option<u32>,
    pub hardware_rates: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SharedPath {
    pub stream_rate: u32,
    pub stream_channels: u16,
    pub stream_volume: f32,
    pub sink: Option<SinkPath>,
    pub sink_volume: Option<f32>,
    pub clock_rate: Option<u32>,
    pub forced_rate: Option<u32>,
    pub allowed_rates: Vec<u32>,
}

impl SharedPath {
    #[must_use]
    pub fn graph_rate(&self) -> Option<u32> {
        self.sink
            .as_ref()
            .and_then(|sink| sink.rate)
            .or(self.forced_rate)
            .or(self.clock_rate)
    }

    #[must_use]
    pub fn stages(&self) -> Vec<Stage> {
        let mut stages = vec![Stage::Volume {
            level: self.stream_volume,
            by: VolumeControl::PipeWire,
        }];
        if let Some(rate) = self.graph_rate()
            && rate != self.stream_rate
        {
            stages.push(Stage::Resample {
                from: self.stream_rate,
                to: rate,
                by: Processor::PipeWire,
            });
        }
        if let Some(channels) = self.sink.as_ref().and_then(|sink| sink.channels)
            && channels != self.stream_channels
        {
            stages.push(Stage::Remix {
                from: self.stream_channels,
                to: channels,
                by: Processor::PipeWire,
            });
        }
        if let Some(level) = self.sink_volume.filter(|level| *level != 1.0) {
            stages.push(Stage::Volume {
                level,
                by: VolumeControl::Sink,
            });
        }
        stages
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExclusivePath {
    pub device: DeviceId,
    pub name: String,
    pub rate: u32,
    pub channels: u16,
    pub format: SampleFormat,
    pub dop: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OutputPath {
    Shared(SharedPath),
    Exclusive(ExclusivePath),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Verdict {
    Lossy,
    HighQuality,
    Lossless,
    BitPerfect,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignalPath {
    pub source: SourceFormat,
    pub stages: Vec<Stage>,
    pub output: OutputPath,
}

impl SignalPath {
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        let bits = self.source.pcm_bits();
        let lossy = self.source.is_lossy()
            || self
                .stages
                .iter()
                .any(|stage| stage.reduces_resolution(bits))
            || self.output_reduces_resolution(bits);
        if lossy {
            return Verdict::Lossy;
        }
        if self.stages.iter().any(Stage::modifies_samples) {
            return Verdict::HighQuality;
        }
        let proven = self.source.encoding.lossless && (bits.is_some() || self.source.is_dsd());
        let untouched = self
            .stages
            .iter()
            .all(|stage| matches!(stage, Stage::Volume { .. } | Stage::Dop));
        match &self.output {
            OutputPath::Exclusive(_) if proven && untouched => Verdict::BitPerfect,
            _ => Verdict::Lossless,
        }
    }

    fn output_reduces_resolution(&self, bits: Option<u32>) -> bool {
        let keeps_precision = |format: SampleFormat| {
            format.is_dsd() || bits.is_none_or(|bits| format.precision() >= bits)
        };
        match &self.output {
            OutputPath::Shared(shared) => shared.sink.as_ref().is_some_and(|sink| {
                sink.bluetooth.is_some()
                    || sink.format.is_some_and(|format| !keeps_precision(format))
            }),
            OutputPath::Exclusive(exclusive) => !keeps_precision(exclusive.format),
        }
    }
}
