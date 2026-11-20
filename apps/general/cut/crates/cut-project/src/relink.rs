use crate::edit::Applied;
use crate::error::{Error, Result};
use crate::media::{Media, MediaId, Stream};
use crate::project::Project;

#[derive(Clone, Debug, PartialEq)]
pub struct Relink {
    pub media: MediaId,
    pub to: Media,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Mismatch {
    #[error("it has no picture")]
    MissingPicture,
    #[error("it has a picture the original lacks")]
    UnexpectedPicture,
    #[error("it has no sound")]
    MissingSound,
    #[error("it lasts {frames} frames but {needed} are used")]
    TooShort { needed: i64, frames: i64 },
}

impl Project {
    #[must_use]
    pub fn uses(&self, media: MediaId) -> usize {
        self.clips()
            .filter(|(_, clip)| clip.media() == Some(media))
            .count()
    }

    fn needed_frames(&self, media: MediaId) -> i64 {
        self.clips()
            .filter(|(_, clip)| clip.media() == Some(media))
            .map(|(_, clip)| clip.source_in() + clip.source_span())
            .max()
            .unwrap_or(0)
    }

    pub fn relink_fit(&self, media: MediaId, to: &Media) -> Result<()> {
        let original = self.media(media).ok_or(Error::UnknownMedia)?;
        let mismatch = |reason| Error::Mismatch {
            path: to.path().to_path_buf(),
            reason,
        };
        match (original.has(Stream::Picture), to.has(Stream::Picture)) {
            (true, false) => return Err(mismatch(Mismatch::MissingPicture)),
            (false, true) => return Err(mismatch(Mismatch::UnexpectedPicture)),
            _ => {}
        }
        if original.has(Stream::Sound) && !to.has(Stream::Sound) {
            return Err(mismatch(Mismatch::MissingSound));
        }
        let needed = self.needed_frames(media);
        let frames = to.frames(self.format.rate);
        match frames < needed {
            true => Err(mismatch(Mismatch::TooShort { needed, frames })),
            false => Ok(()),
        }
    }

    pub(crate) fn relink(&mut self, relinks: &[Relink]) -> Result<Applied> {
        for relink in relinks {
            self.relink_fit(relink.media, &relink.to)?;
        }
        for relink in relinks {
            self.media.insert(relink.media, relink.to.clone());
        }
        Ok(Applied::default())
    }
}
