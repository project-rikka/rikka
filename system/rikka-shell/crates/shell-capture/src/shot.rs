use std::path::{Path, PathBuf};
use std::sync::Arc;

use rikka_clipboard::Clipboard;
use rikka_dirs::UserDir;
use rikka_image::Image;
use rikka_seam::SeamClient;
use rikka_types::Rect;
use shell_kit::{detached, pictures};
use shell_markup::Editing;
use shell_notifications::{ActionRequest, Announcement};

use crate::error::{Error, Result};
use crate::frozen::Frozen;
use crate::preferences::Picture;
use crate::snip::{Snapshot, Snip, Taken};
use crate::store;

const FOLDER: &str = "Screenshots";

const STEM: &str = "Screenshot";

#[derive(Clone, Debug)]
pub(crate) struct Strings {
    pub(crate) app_name: String,
    pub(crate) saved: String,
    pub(crate) copied: String,
    pub(crate) failed: String,
}

pub(super) struct ShotRequest {
    pub(crate) frozen: Arc<Frozen>,
    pub(crate) region: Rect,
    pub(crate) picture: Picture,
    pub(crate) clipboard: Option<Arc<Clipboard>>,
    pub(crate) editing: Editing,
    pub(crate) words: Strings,
    pub(crate) seam: SeamClient,
}

struct SavedShot {
    file: Option<PathBuf>,
    copied: bool,
    action_request: ActionRequest,
}

pub(crate) fn spawn(request: ShotRequest) {
    detached("rikka-capture-shot", move || {
        let seam = request.seam.clone();
        let words = request.words.clone();
        match keep(&request) {
            Ok(outcome) => {
                if let Some(announcement) = announcement(&words, &outcome) {
                    announcement.send(&seam);
                }
            }
            Err(error) => {
                tracing::warn!("failed to save screenshot: {error}");
                Announcement::new(&words.app_name, &words.failed)
                    .body(error.to_string())
                    .urgent()
                    .send(&seam);
            }
        }
    });
}

pub(crate) fn snip(frozen: Arc<Frozen>, region: Rect, snip: Snip, seam: SeamClient) {
    detached("rikka-capture-snip", move || {
        let taken = match cut(&frozen, region) {
            Ok(snapshot) => Taken::Snipped(snapshot),
            Err(error) => Taken::Failed(error),
        };
        snip.deliver(taken, &seam);
    });
}

fn cut(frozen: &Frozen, region: Rect) -> Result<Snapshot> {
    Snapshot::from_image(&frozen.crop(region)?)
}

fn keep(request: &ShotRequest) -> Result<SavedShot> {
    let image = request.frozen.crop(request.region)?;
    let destination = destination(&request.picture)?;
    let file = match request.picture.save {
        true => {
            write(&image, &request.picture, &destination)?;
            Some(destination.clone())
        }
        false => None,
    };
    let copied = match (request.picture.copy, &request.clipboard) {
        (true, Some(clipboard)) => {
            let png = pictures::png(&image)?;
            clipboard.set_png(png)?;
            true
        }
        (true, None) if file.is_some() => {
            tracing::warn!("{}", Error::NoClipboard);
            false
        }
        (true, None) => return Err(Error::NoClipboard),
        (false, _) => false,
    };
    tracing::info!(
        "captured {}x{} to {}",
        image.width(),
        image.height(),
        file.as_deref().map_or_else(
            || "the clipboard".to_owned(),
            |file| file.display().to_string()
        )
    );
    let unsaved = file.is_none().then(|| Arc::new(image));
    let action_request = request
        .editing
        .keep(unsaved, destination, request.picture.quality);
    Ok(SavedShot {
        file,
        copied,
        action_request,
    })
}

fn destination(picture: &Picture) -> Result<PathBuf> {
    let folder = store::folder(picture.folder.as_deref(), UserDir::Pictures, FOLDER)?;
    Ok(store::file(&folder, STEM, picture.format.extension()))
}

fn write(image: &Image, picture: &Picture, file: &Path) -> Result<()> {
    if let Some(folder) = file.parent() {
        store::ensure(folder)?;
    }
    image
        .write(picture.format)
        .quality(picture.quality)
        .to(file)?;
    Ok(())
}

fn announcement(words: &Strings, outcome: &SavedShot) -> Option<Announcement> {
    let label = match (&outcome.file, outcome.copied) {
        (Some(file), _) => Announcement::new(&words.app_name, &words.saved)
            .file(file)
            .image(file.display().to_string()),
        (None, true) => Announcement::new(&words.app_name, &words.copied),
        (None, false) => return None,
    };
    Some(label.action_request(outcome.action_request.clone()))
}
