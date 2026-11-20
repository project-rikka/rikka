use std::path::Path;
use std::sync::Arc;

use replay_ring::Clip;
use rikka_video::{Coded, Muxer, StreamInfo};

use crate::error::{Error, Result};

pub(crate) struct Recording {
    pub(crate) clip: Clip,
    pub(crate) pictures: Arc<StreamInfo>,
    pub(crate) sounds: Vec<Option<Arc<StreamInfo>>>,
}

impl Recording {
    #[must_use]
    pub(crate) fn seconds(&self) -> f64 {
        self.clip.length_us as f64 / 1e6
    }

    #[must_use]
    pub(crate) fn bytes(&self) -> u64 {
        let byte_size = |packets: &[Coded]| -> u64 {
            packets.iter().map(|packet| packet.data.len() as u64).sum()
        };
        byte_size(&self.clip.pictures)
            + self
                .clip
                .sounds
                .iter()
                .map(|sound| byte_size(sound))
                .sum::<u64>()
    }
}

pub(crate) fn write(recording: &Recording, file: &Path) -> Result<()> {
    let written = |error| Error::Write(file.to_path_buf(), error);
    let staged = rikka_fs::Replace::new(file)
        .keep_extension()
        .stage()
        .map_err(written)?;
    mux(recording, staged.path())?;
    staged.commit().map_err(written)
}

fn mux(recording: &Recording, file: &Path) -> Result<()> {
    let audio_tracks: Vec<(&[Coded], &StreamInfo)> = recording
        .clip
        .sounds
        .iter()
        .zip(&recording.sounds)
        .filter_map(|(packets, stream)| {
            let stream = stream.as_deref()?;
            (!packets.is_empty()).then_some((packets.as_slice(), stream))
        })
        .collect();
    let streams: Vec<&StreamInfo> = std::iter::once(&*recording.pictures)
        .chain(audio_tracks.iter().map(|(_, stream)| *stream))
        .collect();
    let tracks: Vec<&[Coded]> = std::iter::once(recording.clip.pictures.as_slice())
        .chain(audio_tracks.iter().map(|(packets, _)| *packets))
        .collect();
    let mut muxer = Muxer::create(file, &streams)?;
    let mut next = vec![0usize; tracks.len()];
    loop {
        let earliest = tracks
            .iter()
            .enumerate()
            .filter_map(|(track, packets)| Some((track, packets.get(next[track])?)))
            .min_by_key(|(_, packet)| packet.dts_us);
        let Some((track, packet)) = earliest else {
            break;
        };
        muxer.write(track, packet.sample())?;
        next[track] += 1;
    }
    muxer.finish()?;
    Ok(())
}

pub(crate) fn in_background(job: impl FnOnce() + Send + 'static) {
    let started = std::thread::Builder::new()
        .name("rikka-replay-save".into())
        .spawn(job);
    if let Err(error) = started {
        tracing::warn!("cannot spawn writer thread: {error}");
    }
}
