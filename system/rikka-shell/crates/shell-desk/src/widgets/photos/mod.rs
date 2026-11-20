mod album;
mod order;
mod shelf;

use std::convert::Infallible;
use std::path::Path;
use std::sync::Arc;

use desk_layout::{PhotoInterval, Photos, Size, WidgetId};
use rikka_theme::Corner;
use rikka_types::Size as Extent;
use rikka_ui::{
    Element, Fetch, Label, Length, MaterialSymbol, Space, Styled, View, fade, fetch_blocking,
    motion, tr,
};
use rikka_widgets::{ImageFit, column_of, container, image};
pub(crate) use shelf::Shelf;
use shell_kit::Period;
use shell_kit::pictures::{self, Developed, Within};

use crate::widgets::live::Live;
use crate::widgets::photos::album::{Album, scan_album};
use crate::widgets::photos::order::pick;
use crate::widgets::quiet::quiet;
use crate::widgets::sources::Sources;

const ALBUM: &str = "desk.photos.album";
const STILL: &str = "desk.photos.still";
const RESCAN: u64 = 600;
const SHARPNESS: f32 = 2.0;
const MAT: Space = Space::SNUG;

#[must_use]
pub(crate) fn view(id: WidgetId, photos: &Photos, size: Size, sources: &Sources) -> Element {
    let slot = sources.feeds.wall.slot(period(photos.interval));
    let Some(album) = album(photos, slot, sources) else {
        return quiet(
            MaterialSymbol::PHOTO_LIBRARY,
            tr!("shell.desk.widget.photos.looking"),
        )
        .sized(size);
    };
    if !album.found {
        return quiet(
            MaterialSymbol::FOLDER_OFF,
            tr!("shell.desk.widget.photos.missing"),
        )
        .detail(folder_name(album.folder.as_deref()))
        .sized(size);
    }
    let (Some(current), Some(next)) = (
        pick(&album.pictures, id, slot, photos.shuffle),
        pick(&album.pictures, id, slot + 1, photos.shuffle),
    ) else {
        return quiet(
            MaterialSymbol::PHOTO_LIBRARY,
            tr!("shell.desk.widget.photos.empty"),
        )
        .detail(folder_name(album.folder.as_deref()))
        .sized(size);
    };
    let extent = size.extent();
    let within = (
        (extent.width * SHARPNESS).ceil() as u32,
        (extent.height * SHARPNESS).ceil() as u32,
    );
    let develop = |picture: &Path| developed(picture, within, photos.fill);
    develop(next);
    let pending = develop(current);
    let shown = match &pending {
        Fetch::Ready(still) => {
            sources.shelf.show(id, current.to_path_buf());
            Some((current.to_path_buf(), Arc::clone(still)))
        }
        Fetch::Loading | Fetch::Failed(_) => sources.shelf.shown(id).and_then(|before| {
            let still = develop(&before).ready().cloned()?;
            Some((before, Arc::new(still)))
        }),
    };
    match (shown, pending) {
        (Some((picture, still)), _) => framed(&picture, &still, extent, photos.fill),
        (None, Fetch::Failed(_)) => quiet(
            MaterialSymbol::HIDE_IMAGE,
            tr!("shell.desk.widget.photos.unreadable"),
        )
        .sized(size),
        (None, Fetch::Loading | Fetch::Ready(_)) => Element::empty(),
    }
}

fn album(photos: &Photos, slot: u64, sources: &Sources) -> Option<Arc<Album>> {
    let epoch = slot * u64::from(photos.interval.secs()) / RESCAN;
    let folder = photos.folder.clone();
    let looked = fetch_blocking((ALBUM, photos.folder.clone(), epoch), move || {
        Ok::<_, Infallible>(scan_album(folder.as_deref()))
    });
    match looked {
        Fetch::Ready(album) => {
            sources.shelf.keep(&photos.folder, &album);
            Some(album)
        }
        Fetch::Loading | Fetch::Failed(_) => sources.shelf.album(&photos.folder),
    }
}

fn developed(picture: &Path, within: (u32, u32), covering: bool) -> Fetch<Developed> {
    let owned = picture.to_path_buf();
    fetch_blocking(
        (STILL, picture.to_path_buf(), within, covering),
        move || {
            let fit = match covering {
                true => Within::Cover,
                false => Within::Contain,
            };
            pictures::develop(&owned, Some(within), fit)
        },
    )
}

fn framed(picture: &Path, still: &Developed, extent: Extent, covering: bool) -> Element {
    let (width, height) = (Length::px(extent.width), Length::px(extent.height));
    let drawn = match covering {
        true => image(still.source())
            .fit(ImageFit::Cover)
            .radius(Corner::Window)
            .width(width)
            .height(height)
            .build(),
        false => {
            let room = Extent::new(
                extent.width - MAT.px() * 2.0,
                extent.height - MAT.px() * 2.0,
            );
            let scale = (room.width / still.width as f32).min(room.height / still.height as f32);
            container(
                image(still.source())
                    .fit(ImageFit::Cover)
                    .radius(Corner::Panel)
                    .width(Length::px(still.width as f32 * scale))
                    .height(Length::px(still.height as f32 * scale)),
            )
            .center()
            .width(width)
            .height(height)
            .build()
        }
    };
    let picture_path = picture.to_path_buf();
    container(
        column_of([drawn
            .keyed(picture)
            .transition(fade().enter(motion::UNVEIL).exit(motion::UNVEIL))
            .position(0.0, 0.0)])
        .width(width)
        .height(height),
    )
    .on_press(move |live: &mut Live| live.show(picture_path.clone()))
    .width(width)
    .height(height)
    .build()
}

#[must_use]
pub(super) fn folder_name(folder: Option<&Path>) -> Label {
    match folder.and_then(Path::file_name) {
        Some(name) => Label::from(name.to_string_lossy().into_owned()),
        None => tr!("shell.desk.widget.photos.pictures"),
    }
}

#[must_use]
pub(crate) fn period(interval: PhotoInterval) -> Period {
    match interval {
        PhotoInterval::HalfMinute => Period::HalfMinute,
        PhotoInterval::Minute => Period::Minute,
        PhotoInterval::FiveMinutes => Period::FiveMinutes,
        PhotoInterval::HalfHour => Period::HalfHour,
        PhotoInterval::Hour => Period::Hour,
    }
}
