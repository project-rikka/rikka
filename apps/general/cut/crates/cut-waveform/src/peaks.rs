use std::ops::Range;

pub(crate) const BASE: u64 = 64;
const FANOUT: u64 = 4;
const LEVELS: usize = 10;
const PER_SUMMARY: u64 = 4;
const SAMPLE_SCALE: f32 = i16::MAX as f32;
const RMS_SCALE: f32 = u16::MAX as f32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Bucket {
    pub(crate) low: i16,
    pub(crate) high: i16,
    pub(crate) rms: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Summary {
    pub low: f32,
    pub high: f32,
    pub rms: f32,
}

#[derive(Clone, Copy, Debug)]
struct Accumulator {
    low: f32,
    high: f32,
    squares: f64,
    count: u64,
}

impl Accumulator {
    const EMPTY: Accumulator = Accumulator {
        low: f32::INFINITY,
        high: f32::NEG_INFINITY,
        squares: 0.0,
        count: 0,
    };

    fn add_sample(&mut self, value: f32) {
        self.low = self.low.min(value);
        self.high = self.high.max(value);
        self.squares += f64::from(value) * f64::from(value);
        self.count += 1;
    }

    fn add_bucket(&mut self, bucket: Bucket) {
        let rms = f64::from(bucket.rms) / f64::from(RMS_SCALE);
        self.low = self.low.min(f32::from(bucket.low) / SAMPLE_SCALE);
        self.high = self.high.max(f32::from(bucket.high) / SAMPLE_SCALE);
        self.squares += rms * rms;
        self.count += 1;
    }

    fn summary(&self) -> Option<Summary> {
        (self.count > 0).then(|| Summary {
            low: self.low,
            high: self.high,
            rms: (self.squares / self.count as f64).sqrt() as f32,
        })
    }

    fn to_bucket(self) -> Bucket {
        let quantized = |value: f32| (value.clamp(-1.0, 1.0) * SAMPLE_SCALE).round() as i16;
        let rms = (self.squares / self.count.max(1) as f64).sqrt() as f32;
        Bucket {
            low: quantized(self.low),
            high: quantized(self.high),
            rms: (rms.clamp(0.0, 1.0) * RMS_SCALE).round() as u16,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Peaks {
    rate: u32,
    channels: u16,
    frames: u64,
    expected: Option<u64>,
    complete: bool,
    levels: Vec<Vec<Bucket>>,
    pending: Vec<Accumulator>,
}

impl Peaks {
    #[must_use]
    pub fn new(rate: u32, channels: u16) -> Peaks {
        let channels = channels.max(1);
        Peaks {
            rate,
            channels,
            frames: 0,
            expected: None,
            complete: false,
            levels: vec![Vec::new()],
            pending: vec![Accumulator::EMPTY; usize::from(channels)],
        }
    }

    #[must_use]
    pub fn with_expected_frames(mut self, frames: Option<u64>) -> Peaks {
        self.expected = frames;
        self
    }

    pub(crate) fn from_base(rate: u32, channels: u16, frames: u64, base: Vec<Bucket>) -> Peaks {
        let mut peaks = Peaks::new(rate, channels);
        peaks.frames = frames;
        peaks.expected = Some(frames);
        peaks.levels[0] = base;
        peaks.complete = true;
        while peaks.build_next_level() {}
        peaks
    }

    #[must_use]
    pub fn rate(&self) -> u32 {
        self.rate
    }

    #[must_use]
    pub fn channels(&self) -> u16 {
        self.channels
    }

    #[must_use]
    pub fn frames(&self) -> u64 {
        self.frames
    }

    #[must_use]
    pub fn expected(&self) -> Option<u64> {
        self.expected
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.complete
    }

    pub(crate) fn base(&self) -> &[Bucket] {
        &self.levels[0]
    }

    pub fn extend(&mut self, interleaved: &[f32]) {
        let channels = usize::from(self.channels);
        for frame in interleaved.chunks_exact(channels) {
            for (accumulator, sample) in self.pending.iter_mut().zip(frame) {
                accumulator.add_sample(*sample);
            }
            self.frames += 1;
            if self.frames.is_multiple_of(BASE) {
                self.flush_pending();
            }
        }
    }

    pub fn finish(&mut self) {
        if self.pending.iter().any(|accumulator| accumulator.count > 0) {
            self.flush_pending();
        }
        self.complete = true;
        self.expected = Some(self.frames);
        while self.build_next_level() {}
    }

    fn flush_pending(&mut self) {
        let flushed: Vec<Bucket> = self
            .pending
            .iter()
            .copied()
            .map(Accumulator::to_bucket)
            .collect();
        self.levels[0].extend(flushed);
        self.pending.fill(Accumulator::EMPTY);
        while self.build_next_level() {}
    }

    fn build_next_level(&mut self) -> bool {
        let channels = usize::from(self.channels);
        let fan = FANOUT as usize;
        for level in 0..LEVELS - 1 {
            if self.levels.len() == level + 1 {
                self.levels.push(Vec::new());
            }
            let below = self.levels[level].len() / channels;
            let above = self.levels[level + 1].len() / channels;
            let whole = below / fan;
            let ready = match self.complete {
                true => below.div_ceil(fan),
                false => whole,
            };
            if ready > above {
                let built: Vec<Bucket> = (above..ready)
                    .flat_map(|group| {
                        let children = &self.levels[level];
                        (0..channels).map(move |channel| {
                            let mut accumulator = Accumulator::EMPTY;
                            for child in group * fan..((group + 1) * fan).min(below) {
                                accumulator.add_bucket(children[child * channels + channel]);
                            }
                            accumulator.to_bucket()
                        })
                    })
                    .collect();
                self.levels[level + 1].extend(built);
                return true;
            }
        }
        false
    }

    #[must_use]
    pub fn summary(&self, channel: u16, frames: Range<u64>) -> Option<Summary> {
        let channel = usize::from(channel.min(self.channels - 1));
        let mut accumulator = Accumulator::EMPTY;
        self.visit(&frames, &mut |bucket: &[Bucket]| {
            accumulator.add_bucket(bucket[channel]);
        });
        accumulator.summary()
    }

    #[must_use]
    pub fn mixed(&self, frames: Range<u64>) -> Option<Summary> {
        let mut accumulator = Accumulator::EMPTY;
        self.visit(&frames, &mut |buckets: &[Bucket]| {
            for bucket in buckets {
                accumulator.add_bucket(*bucket);
            }
        });
        accumulator.summary()
    }

    fn visit(&self, frames: &Range<u64>, each: &mut impl FnMut(&[Bucket])) {
        let end = frames.end.min(self.frames);
        if frames.start >= end {
            return;
        }
        let span = end - frames.start;
        let level = (0..self.levels.len())
            .rev()
            .find(|level| BASE * FANOUT.pow(*level as u32) * PER_SUMMARY <= span)
            .unwrap_or(0);
        self.visit_level(level, frames.start..end, each);
    }

    fn visit_level(&self, level: usize, frames: Range<u64>, each: &mut impl FnMut(&[Bucket])) {
        let channels = usize::from(self.channels);
        let width = BASE * FANOUT.pow(level as u32);
        let buckets = &self.levels[level];
        let available = (buckets.len() / channels) as u64;
        let first = frames.start / width;
        let past = frames.end.div_ceil(width).min(available);
        for index in first..past {
            let at = index as usize * channels;
            each(&buckets[at..at + channels]);
        }
        let covered = (past * width).max(frames.start);
        if covered < frames.end && level > 0 {
            self.visit_level(level - 1, covered..frames.end, each);
        }
    }
}
