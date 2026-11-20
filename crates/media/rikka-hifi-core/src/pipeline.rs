use rikka_resample::{ResampleQuality, Resampler};

use crate::dop::DopPacker;
use crate::dsd_packer::DsdPacker;
use crate::dsd_pcm::DsdToPcm;
use crate::gain::amplitude;
use crate::path::{Processor, Stage, VolumeControl};
use crate::plan::{Carriage, Plan};
use crate::quantize::Quantizer;
use crate::remix::Remix;
use crate::source_format::{Resolution, SourceFormat};

const QUALITY: ResampleQuality = ResampleQuality::Ultra;

struct Resampling {
    resampler: Resampler,
    from: u32,
    to: u32,
}

pub struct Pipeline {
    source: SourceFormat,
    plan: Plan,
    decimator: Option<DsdToPcm>,
    resampling: Option<Resampling>,
    remix: Option<Remix>,
    replay_gain_db: f32,
    volume: Option<f32>,
    quantizer: Option<Quantizer>,
    dop: Option<DopPacker>,
    native: Option<DsdPacker>,
    decoded: Vec<f32>,
    resampled: Vec<f32>,
    remixed: Vec<f32>,
    gained: Vec<f32>,
}

impl Pipeline {
    #[must_use]
    pub fn new(source: &SourceFormat, plan: Plan) -> Pipeline {
        let (decimator, pcm_rate) = match (plan.carriage, source.resolution) {
            (Carriage::DsdToPcm, Resolution::Dsd { rate }) => {
                let decimator = DsdToPcm::new(rate, source.channels);
                let pcm_rate = decimator.rate();
                (Some(decimator), pcm_rate)
            }
            (_, Resolution::Pcm { rate, .. }) => (None, rate),
            (_, Resolution::Dsd { .. }) => (None, plan.rate),
        };
        let pcm = matches!(plan.carriage, Carriage::Pcm | Carriage::DsdToPcm);
        let resampling = (pcm && pcm_rate != plan.rate).then(|| Resampling {
            resampler: Resampler::with_quality(QUALITY, pcm_rate, plan.rate, source.channels),
            from: pcm_rate,
            to: plan.rate,
        });
        let remix = (pcm && source.channels != plan.channels)
            .then(|| Remix::new(source.channels, plan.channels));
        let mut pipeline = Pipeline {
            source: *source,
            plan,
            decimator,
            resampling,
            remix,
            replay_gain_db: 0.0,
            volume: None,
            quantizer: pcm.then(|| Quantizer::new(plan.format)),
            dop: (plan.carriage == Carriage::Dop)
                .then(|| DopPacker::new(plan.channels, plan.format)),
            native: (plan.carriage == Carriage::NativeDsd)
                .then(|| DsdPacker::new(plan.channels, plan.format)),
            decoded: Vec::new(),
            resampled: Vec::new(),
            remixed: Vec::new(),
            gained: Vec::new(),
        };
        pipeline.redither();
        pipeline
    }

    #[must_use]
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    #[must_use]
    pub fn source(&self) -> &SourceFormat {
        &self.source
    }

    #[must_use]
    pub fn takes_gain(&self) -> bool {
        self.quantizer.is_some()
    }

    pub fn set_replay_gain(&mut self, db: f32) {
        self.replay_gain_db = if self.takes_gain() { db } else { 0.0 };
        self.redither();
    }

    pub fn set_volume(&mut self, level: Option<f32>) {
        self.volume = level
            .filter(|_| self.takes_gain())
            .map(|level| level.clamp(0.0, 1.0));
        self.redither();
    }

    pub fn pcm(&mut self, samples: &[f32], out: &mut Vec<u8>) {
        self.shape(samples, false, out);
    }

    pub fn dsd(&mut self, bytes: &[u8], out: &mut Vec<u8>) {
        if let Some(native) = &mut self.native {
            native.pack(bytes, out);
        } else if let Some(dop) = &mut self.dop {
            dop.pack(bytes, out);
        } else if let Some(decimator) = &mut self.decimator {
            let mut decoded = std::mem::take(&mut self.decoded);
            decoded.clear();
            decimator.process(bytes, &mut decoded);
            self.shape(&decoded, false, out);
            self.decoded = decoded;
        }
    }

    pub fn finish(&mut self, out: &mut Vec<u8>) {
        if let Some(native) = &mut self.native {
            native.finish(out);
        }
        if let Some(dop) = &mut self.dop {
            dop.finish(out);
        }
        if self.quantizer.is_some() {
            self.shape(&[], true, out);
        }
    }

    pub fn reset(&mut self) {
        if let Some(resampling) = &mut self.resampling {
            resampling.resampler.reset();
        }
        if let Some(decimator) = &mut self.decimator {
            decimator.reset();
        }
        if let Some(dop) = &mut self.dop {
            dop.reset();
        }
        if let Some(native) = &mut self.native {
            native.reset();
        }
    }

    #[must_use]
    pub fn stages(&self) -> Vec<Stage> {
        let mut stages = Vec::new();
        if let (Some(decimator), Resolution::Dsd { rate }) =
            (&self.decimator, self.source.resolution)
        {
            stages.push(Stage::DsdToPcm {
                from: rate,
                to: decimator.rate(),
            });
        }
        if let (Some(from), Resolution::Pcm { rate, .. }) =
            (self.source.encoding.dsd_rate, self.source.resolution)
        {
            stages.push(Stage::DsdToPcm { from, to: rate });
        }
        if let Some(resampling) = &self.resampling {
            stages.push(Stage::Resample {
                from: resampling.from,
                to: resampling.to,
                by: Processor::Rikka,
            });
        }
        if let Some(remix) = &self.remix {
            stages.push(Stage::Remix {
                from: remix.from(),
                to: remix.to(),
                by: Processor::Rikka,
            });
        }
        if self.replay_gain_db != 0.0 {
            stages.push(Stage::ReplayGain {
                db: self.replay_gain_db,
            });
        }
        if let Some(level) = self.volume {
            stages.push(Stage::Volume {
                level,
                by: VolumeControl::Software,
            });
        }
        if let Some(quantizer) = self.quantizer.as_ref().filter(|q| q.is_dithered()) {
            stages.push(Stage::Dither {
                bits: quantizer.bits(),
            });
        }
        if self.dop.is_some() {
            stages.push(Stage::Dop);
        }
        stages
    }

    fn gain(&self) -> f32 {
        amplitude(self.replay_gain_db) * self.volume.unwrap_or(1.0)
    }

    fn modifies_samples(&self) -> bool {
        let requantizes = self.quantizer.as_ref().is_some_and(|quantizer| {
            self.source
                .pcm_bits()
                .is_none_or(|bits| bits > quantizer.bits())
        });
        self.gain() != 1.0
            || self.resampling.is_some()
            || self.decimator.is_some()
            || self.remix.is_some_and(|remix| remix.mixes())
            || requantizes
    }

    fn redither(&mut self) {
        let altered = self.modifies_samples();
        if let Some(quantizer) = &mut self.quantizer {
            quantizer.set_dithered(altered);
        }
    }

    fn shape(&mut self, samples: &[f32], last: bool, out: &mut Vec<u8>) {
        let gain = self.gain();
        let Pipeline {
            resampling,
            remix,
            quantizer,
            resampled,
            remixed,
            gained,
            ..
        } = self;
        let Some(quantizer) = quantizer else {
            return;
        };
        let mut current = samples;
        if let Some(resampling) = resampling {
            resampled.clear();
            resampling.resampler.process(current, resampled);
            if last {
                resampling.resampler.flush(resampled);
            }
            current = resampled;
        }
        if let Some(remix) = remix {
            remixed.clear();
            remix.apply(current, remixed);
            current = remixed;
        }
        if gain != 1.0 {
            gained.clear();
            gained.extend(current.iter().map(|sample| sample * gain));
            current = gained;
        }
        quantizer.write(current, out);
    }
}

impl std::fmt::Debug for Pipeline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pipeline")
            .field("source", &self.source)
            .field("plan", &self.plan)
            .field("stages", &self.stages())
            .finish_non_exhaustive()
    }
}
