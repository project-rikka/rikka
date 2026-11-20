mod picture;
mod sound;
mod still;
mod video;

pub use picture::{FrameRate, Resolution};
pub use sound::{AAC_BITRATES, Audio, SoundCodec};
pub use still::{Still, StillFormat};
pub use video::{Compression, Container, Preset, Video};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Target {
    #[default]
    Video,
    Audio,
    Still,
}

impl Target {
    pub const ALL: [Target; 3] = [Target::Video, Target::Audio, Target::Still];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub target: Target,
    pub video: Video,
    pub audio: Audio,
    pub still: Still,
}

impl Settings {
    #[must_use]
    pub fn extension(&self) -> &'static str {
        match self.target {
            Target::Video => self.video.container.extension(),
            Target::Audio => self.audio.codec.extension(),
            Target::Still => self.still.format.extension(),
        }
    }

    #[must_use]
    pub fn mime_type(&self) -> &'static str {
        match self.target {
            Target::Video => self.video.container.mime_type(),
            Target::Audio => self.audio.codec.mime_type(),
            Target::Still => self.still.format.mime_type(),
        }
    }
}
