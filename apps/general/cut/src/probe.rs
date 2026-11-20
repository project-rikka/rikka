use std::path::Path;

use cut_project::{Media, Picture, Rate, Sound};
use rikka_media_format::Kind;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ProbeError {
    #[error("file is a still image")]
    StillImage,
    #[error("file has no video or audio stream")]
    Unsupported,
    #[error("unknown duration")]
    UnknownDuration,
    #[error("unknown frame rate")]
    UnknownFrameRate,
    #[error(transparent)]
    Video(#[from] rikka_video::Error),
    #[error(transparent)]
    Audio(#[from] rikka_audio_decode::Error),
}

pub(crate) fn footage(path: &Path) -> Result<Media, ProbeError> {
    match rikka_media_format::kind(path) {
        Some(Kind::Picture) => Err(ProbeError::StillImage),
        Some(Kind::Sound) => sound_only(path),
        Some(Kind::Movie) => movie(path),
        None => movie(path)
            .or_else(|_| sound_only(path))
            .map_err(|_| ProbeError::Unsupported),
    }
}

fn movie(path: &Path) -> Result<Media, ProbeError> {
    let info = rikka_video::probe(path)?;
    let sound = rikka_audio_decode::read(path).probe().ok();
    let duration = info
        .duration
        .or_else(|| sound.and_then(|sound| sound.duration))
        .ok_or(ProbeError::UnknownDuration)?;
    let picture = Picture {
        width: info.width,
        height: info.height,
        rate: Rate::measured(info.frame_rate).ok_or(ProbeError::UnknownFrameRate)?,
    };
    let media = Media::new(path, duration).with_picture(picture);
    Ok(match sound {
        Some(sound) => media.with_sound(Sound {
            sample_rate: sound.sample_rate,
            channels: sound.channels,
        }),
        None => media,
    })
}

fn sound_only(path: &Path) -> Result<Media, ProbeError> {
    let info = rikka_audio_decode::read(path).probe()?;
    let duration = info.duration.ok_or(ProbeError::UnknownDuration)?;
    Ok(Media::new(path, duration).with_sound(Sound {
        sample_rate: info.sample_rate,
        channels: info.channels,
    }))
}
