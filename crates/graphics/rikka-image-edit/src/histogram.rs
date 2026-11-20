use bytemuck::{Pod, Zeroable};

use crate::error::Result;
use crate::picture::Picture;
use crate::plane::Plane;
use crate::shader::Shader;
use crate::work::{Io, PushHeader, Work};

pub const BINS: usize = 1024;

const COUNT: u32 = 0;
const TONE_MEANS: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Channel {
    Red,
    Green,
    Blue,
    Luminosity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Histogram {
    counts: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ToneMeans {
    pub(crate) dark: Option<[f32; 3]>,
    pub(crate) light: Option<[f32; 3]>,
    pub(crate) neutral: Option<[f32; 3]>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct HistogramPush {
    head: PushHeader,
    counts: u64,
    dark: f32,
    light: f32,
}

impl Histogram {
    pub(crate) fn from_picture(picture: &Picture) -> Result<Histogram> {
        let mut work = Work::new(picture.imaging());
        Histogram::from_plane(&mut work, &picture.frame().plane)
    }

    pub(crate) fn from_plane(work: &mut Work, plane: &Plane) -> Result<Histogram> {
        let bytes = reduce_to_buffer(work, plane, COUNT, 4 * BINS * 4, [0.0, 0.0])?;
        Ok(Histogram {
            counts: bytemuck::pod_collect_to_vec(&bytes[..4 * BINS * 4]),
        })
    }

    #[must_use]
    pub fn channel(&self, channel: Channel) -> &[u32] {
        let at = channel as usize * BINS;
        &self.counts[at..at + BINS]
    }

    #[must_use]
    pub fn total(&self) -> u64 {
        self.channel(Channel::Luminosity)
            .iter()
            .map(|&count| u64::from(count))
            .sum()
    }

    #[must_use]
    pub fn percentile(&self, channel: Channel, fraction: f32) -> f32 {
        percentile_of(
            self.channel(channel).iter().map(|&count| u64::from(count)),
            fraction,
        )
    }

    #[must_use]
    pub fn mean(&self, channel: Channel) -> f32 {
        let total = self.total();
        if total == 0 {
            return 0.0;
        }
        let sum: f64 = self
            .channel(channel)
            .iter()
            .enumerate()
            .map(|(bin, &count)| (bin as f64 + 0.5) / BINS as f64 * f64::from(count))
            .sum();
        (sum / total as f64) as f32
    }
}

pub(crate) fn percentile_of(bins: impl Iterator<Item = u64> + Clone, fraction: f32) -> f32 {
    let total: u64 = bins.clone().sum();
    if total == 0 {
        return 0.0;
    }
    let target = (f64::from(fraction.clamp(0.0, 1.0)) * total as f64).max(1.0);
    let mut cumulative = 0u64;
    for (bin, count) in bins.enumerate() {
        cumulative += count;
        if cumulative as f64 >= target {
            return bin as f32 / (BINS - 1) as f32;
        }
    }
    1.0
}

fn reduce_to_buffer(
    work: &mut Work,
    plane: &Plane,
    mode: u32,
    bytes: usize,
    [dark, light]: [f32; 2],
) -> Result<Vec<u8>> {
    let reduce_to_buffer = work.zeroed(bytes as u64)?;
    let push = HistogramPush {
        head: PushHeader::new(plane, plane).mode(mode),
        counts: reduce_to_buffer.address(),
        dark,
        light,
    };
    let groups = [plane.width().div_ceil(16), plane.height().div_ceil(16), 1];
    let io = Io::new(plane, plane).write_buffer(&reduce_to_buffer);
    work.groups(
        Shader::Histogram,
        push,
        io,
        groups,
        f64::from(plane.width()) * f64::from(plane.height()),
    )?;
    let download = work.batch().download_buffer(&reduce_to_buffer);
    work.submit()?;
    Ok(download.wait()?)
}

pub(crate) fn tone_means(
    work: &mut Work,
    plane: &Plane,
    dark: f32,
    light: f32,
) -> Result<ToneMeans> {
    let bytes = reduce_to_buffer(work, plane, TONE_MEANS, 24 * 4, [dark, light])?;
    let words: Vec<u32> = bytemuck::pod_collect_to_vec(&bytes);
    let wide = |slot: usize| u64::from(words[slot * 2]) | (u64::from(words[slot * 2 + 1]) << 32);
    let mean = |group: usize| {
        let count = wide(group * 4 + 3);
        (count > 0).then(|| {
            [0, 1, 2]
                .map(|channel| (wide(group * 4 + channel) as f64 / count as f64 / 255.0) as f32)
        })
    };
    Ok(ToneMeans {
        dark: mean(0),
        light: mean(1),
        neutral: mean(2),
    })
}
