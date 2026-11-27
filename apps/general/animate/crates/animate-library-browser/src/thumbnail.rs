use std::path::PathBuf;
use std::sync::Arc;

use animate_library::{Details, Entry, Shelf, Thumbnail, Variant};
use rikka_ui::{Fetch, fetch};

use crate::shelving::{Performer, Thumbnailing};

#[derive(Clone, Debug)]
pub(crate) enum Shown {
    Waiting,
    Picture(Arc<PathBuf>),
    Bare,
}

pub fn variant(entry: &Entry, performer: Option<&Performer>) -> Variant {
    match (entry.shelf(), performer) {
        (Shelf::Motions, Some(performer)) if entry.is_usable() => Variant::Performed(performer.key),
        _ => Variant::Still,
    }
}

pub(crate) fn shown(
    entry: &Arc<Entry>,
    variant: Variant,
    thumbnailing: Option<&Thumbnailing>,
) -> Shown {
    let Some(thumbnailing) = thumbnailing else {
        return Shown::Bare;
    };
    if entry.sidecar().is_none() && matches!(entry.details(), Details::Pending) {
        return Shown::Waiting;
    }
    let key = (
        entry.id(),
        entry.size(),
        entry.modified(),
        variant,
        entry.is_usable(),
    );
    let made = Arc::clone(entry);
    let thumbnailing = thumbnailing.clone();
    match fetch(key, move || made_for(made, variant, thumbnailing)) {
        Fetch::Loading => Shown::Waiting,
        Fetch::Ready(path) => Shown::Picture(path),
        Fetch::Failed(_) => Shown::Bare,
    }
}

async fn made_for(
    entry: Arc<Entry>,
    variant: Variant,
    thumbnailing: Thumbnailing,
) -> Result<PathBuf, String> {
    let thumbnails = Arc::clone(&thumbnailing.thumbnails);
    let looked = Arc::clone(&entry);
    let found = blocking(move || Ok(thumbnails.thumbnail(&looked, variant))).await?;
    match found {
        Thumbnail::Ready(path) => Ok(path),
        Thumbnail::Extract => {
            let thumbnails = Arc::clone(&thumbnailing.thumbnails);
            blocking(move || {
                thumbnails
                    .extract(&entry)
                    .map_err(|error| error.to_string())
            })
            .await
        }
        Thumbnail::Render if entry.is_usable() => {
            let picture = thumbnailing
                .painter
                .paint(Arc::clone(&entry), variant)
                .await?;
            let thumbnails = Arc::clone(&thumbnailing.thumbnails);
            blocking(move || {
                thumbnails
                    .store(
                        &entry,
                        variant,
                        picture.width,
                        picture.height,
                        &picture.rgba,
                    )
                    .map_err(|error| error.to_string())
            })
            .await
        }
        Thumbnail::Render | Thumbnail::Pending | Thumbnail::Missing => Err("no picture".to_owned()),
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())?
}
