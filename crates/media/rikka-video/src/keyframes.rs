use std::path::Path;
use std::time::Duration;

use crate::error::Result;
use rikka_ffmpeg::Packet;
use rikka_ffmpeg::avutil;
use rikka_video_core::demux::Demuxer;
use rikka_video_core::init;

#[derive(Clone, Debug, Default)]
pub struct Keyframes {
    times: Vec<Duration>,
}

impl Keyframes {
    pub fn scan(path: impl AsRef<Path>) -> Result<Keyframes> {
        init::ensure()?;
        let path = path.as_ref();
        let mut demuxer = Demuxer::open(path)?;
        let time_base = demuxer.time_base();
        let mut packet = Packet::new()?;
        let mut times = Vec::new();

        while demuxer.read(&mut packet)? {
            let stamp = packet.is_key().then(|| packet.stamp()).flatten();
            packet.unref();
            if let Some(stamp) = stamp {
                let micros =
                    unsafe { avutil::av_rescale_q(stamp, time_base, avutil::AV_TIME_BASE_Q) };
                times.push(Duration::from_micros(micros.max(0) as u64));
            }
        }

        let keyframes = Keyframes::from_iter(times);
        tracing::debug!(
            "{} can be cut at {} places",
            path.display(),
            keyframes.times.len()
        );
        Ok(keyframes)
    }

    #[must_use]
    pub fn times(&self) -> &[Duration] {
        &self.times
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    #[must_use]
    pub fn nearest(&self, to: Duration) -> Option<Duration> {
        let after = self.times.partition_point(|time| *time <= to);
        let before = after.checked_sub(1).map(|index| self.times[index]);
        match (before, self.times.get(after).copied()) {
            (Some(before), Some(after)) if after - to < to - before => Some(after),
            (Some(before), _) => Some(before),
            (None, after) => after,
        }
    }

    #[must_use]
    pub fn at_or_before(&self, at: Duration) -> Option<Duration> {
        let after = self.times.partition_point(|time| *time <= at);
        after.checked_sub(1).map(|index| self.times[index])
    }
}

impl FromIterator<Duration> for Keyframes {
    fn from_iter<T: IntoIterator<Item = Duration>>(times: T) -> Keyframes {
        let mut times: Vec<Duration> = times.into_iter().collect();
        times.sort_unstable();
        times.dedup();
        Keyframes { times }
    }
}
