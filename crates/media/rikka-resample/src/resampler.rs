use crate::algorithm::ResampleAlgorithm;
use crate::cursor::{Cursor, gcd};
use crate::kernel::{AnyKernel, Kernel};
use crate::planes::Planes;
use crate::polynomial_degree::PolynomialDegree;
use crate::quality::ResampleQuality;

const PASS_FRAMES: usize = 4096;
const COMPACT_THRESHOLD: usize = 4096;
const SPEED_SCALES: [u64; 5] = [10_000, 1000, 100, 10, 1];

pub struct Resampler {
    channels: usize,
    kernel: AnyKernel,
    cursor: Cursor,
    planes: Planes,
    passthrough: bool,
    flushed: bool,
}

impl Resampler {
    #[must_use]
    pub fn with_quality(
        quality: ResampleQuality,
        in_rate: u32,
        out_rate: u32,
        channels: u16,
    ) -> Resampler {
        Resampler::new(quality.algorithm(), in_rate, out_rate, channels)
    }

    #[must_use]
    pub fn at_speed(
        quality: ResampleQuality,
        in_rate: u32,
        out_rate: u32,
        speed: f32,
        channels: u16,
    ) -> Resampler {
        let speed = f64::from(speed);
        let (into, out) = SPEED_SCALES
            .into_iter()
            .find_map(|scale| {
                let into = (f64::from(in_rate) * speed * scale as f64).round() as u64;
                let out = u64::from(out_rate) * scale;
                let common = gcd(into, out).max(1);
                let (into, out) = (into / common, out / common);
                (into > 0 && into <= u64::from(u32::MAX) && out <= u64::from(u32::MAX))
                    .then_some((into as u32, out as u32))
            })
            .unwrap_or((in_rate, out_rate));
        Resampler::with_quality(quality, into, out, channels)
    }

    pub(crate) fn new(
        algorithm: ResampleAlgorithm,
        in_rate: u32,
        out_rate: u32,
        channels: u16,
    ) -> Resampler {
        let in_rate = in_rate.max(1);
        let out_rate = out_rate.max(1);
        let passthrough = in_rate == out_rate;
        let algorithm = if passthrough {
            ResampleAlgorithm::Polynomial(PolynomialDegree::Nearest)
        } else {
            algorithm
        };
        let channels = channels.max(1) as usize;
        let cursor = Cursor::new(in_rate, out_rate);

        let mut resampler = Resampler {
            channels,
            kernel: AnyKernel::new(algorithm, &cursor),
            cursor,
            planes: Planes::new(channels),
            passthrough,
            flushed: false,
        };
        resampler.reset();
        resampler
    }

    pub fn reset(&mut self) {
        let lookbehind = self.kernel.lookbehind();
        self.planes.restart(lookbehind);
        self.cursor.restart(lookbehind);
        self.flushed = false;
    }

    pub fn process(&mut self, input: &[f32], output: &mut Vec<f32>) {
        debug_assert_eq!(input.len() % self.channels, 0);
        debug_assert!(!self.flushed, "input after a flush needs a reset first");
        if self.passthrough {
            output.extend_from_slice(input);
            return;
        }
        for pass in input.chunks(PASS_FRAMES * self.channels) {
            self.planes.push_interleaved(pass);
            self.produce(output);
            self.compact();
        }
    }

    pub fn flush(&mut self, output: &mut Vec<f32>) {
        if self.passthrough || self.flushed {
            return;
        }
        self.flushed = true;
        let end = self.planes.frames();
        self.planes.pad(self.kernel.lookahead() + 1);
        self.kernel
            .render(&self.planes, &mut self.cursor, end, output);
    }

    fn produce(&mut self, output: &mut Vec<f32>) {
        let limit = self.planes.frames().saturating_sub(self.kernel.lookahead());
        self.kernel
            .render(&self.planes, &mut self.cursor, limit, output);
    }

    fn compact(&mut self) {
        let consumed = self.cursor.index() - self.kernel.lookbehind();
        if consumed >= COMPACT_THRESHOLD {
            self.planes.discard(consumed);
            self.cursor.rebase(consumed);
        }
    }
}

impl std::fmt::Debug for Resampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resampler")
            .field("channels", &self.channels)
            .field("passthrough", &self.passthrough)
            .finish_non_exhaustive()
    }
}
