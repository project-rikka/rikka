use std::borrow::Cow;
use std::sync::{Arc, Mutex};

use rikka_image::{Filter, Image};
use rikka_seam::{Request, SeamClient};
use rikka_ui::ImageSource;
use shell_kit::pictures;

use crate::error::{Error, Result};

const LONGEST_SIDE: u32 = 1600;

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub shown: ImageSource,
    pub png: Arc<[u8]>,
}

impl Snapshot {
    pub(super) fn from_image(picture: &Image) -> Result<Snapshot> {
        let shown = ImageSource::Rgba {
            width: picture.width(),
            height: picture.height(),
            pixels: Arc::from(picture.rgba8()),
        };
        let png = pictures::png(fitted(picture)?.as_ref())?;
        Ok(Snapshot {
            shown,
            png: Arc::from(png),
        })
    }
}

#[derive(Debug)]
pub enum Taken {
    Snipped(Snapshot),
    Failed(Error),
}

#[derive(Clone, Debug)]
pub struct Snip {
    slot: Arc<Mutex<Option<Taken>>>,
    request: Request,
}

impl Snip {
    #[must_use]
    pub fn with_request(request: Request) -> Snip {
        Snip {
            slot: Arc::default(),
            request,
        }
    }

    pub(crate) fn deliver(&self, taken: Taken, seam: &SeamClient) {
        if let Ok(mut slot) = self.slot.lock() {
            *slot = Some(taken);
        }
        seam.notify(self.request.clone());
    }

    #[must_use]
    pub fn take(&self) -> Option<Taken> {
        self.slot.lock().ok()?.take()
    }
}

fn fitted(picture: &Image) -> Result<Cow<'_, Image>> {
    let (width, height) = picture.dimensions();
    if width.max(height) <= LONGEST_SIDE {
        return Ok(Cow::Borrowed(picture));
    }
    Ok(Cow::Owned(picture.resize_to_fit(
        LONGEST_SIDE,
        LONGEST_SIDE,
        Filter::Lanczos3,
    )?))
}
