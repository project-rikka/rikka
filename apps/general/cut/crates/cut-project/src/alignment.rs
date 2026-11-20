use crate::clip::Clip;
use crate::media::MediaId;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Alignment {
    pub media: MediaId,
    pub speed: f32,
    pub origin: f64,
}

impl Alignment {
    #[must_use]
    pub fn for_clip(clip: &Clip) -> Option<Alignment> {
        Some(Alignment {
            media: clip.media()?,
            speed: clip.treatment()?.speed,
            origin: clip.origin(),
        })
    }

    #[must_use]
    pub fn continues(&self, other: &Alignment) -> bool {
        self.media == other.media
            && self.speed == other.speed
            && (self.origin - other.origin).abs() < 1.0
    }
}
