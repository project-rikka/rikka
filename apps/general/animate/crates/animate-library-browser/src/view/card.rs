use std::sync::Arc;
use std::time::Duration;

use animate_library::{Entry, Fit, Shelf, Variant};
use animate_ui::Reach;
use rikka_app::prelude::*;
use rikka_picture::Feed;
use rikka_widgets::{ImageFit, column, context_area, image, menu_item, spinner};

use super::lift::lift;
use super::wording;
use crate::shelving::{Shelving, Thumbnailing};
use crate::thumbnail::{self, Shown};

const PREVIEW_DELAY: Duration = Duration::from_millis(280);
const PREVIEW_KEY: &str = "animate.library.preview";
const MISMATCHED: f32 = 0.45;
const BARE_SHARE: f32 = 0.38;
const SELECTED_BORDER: f32 = 2.0;

pub(super) struct Cell {
    pub(super) entry: Arc<Entry>,
    pub(super) look: Look,
    pub(super) feed: Option<Feed>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Look {
    pub(super) tile: f32,
    pub(super) variant: Variant,
    pub(super) selected: bool,
}

pub(super) fn card(
    cell: Cell,
    thumbnailing: Option<&Thumbnailing>,
    reach: Reach<dyn Shelving>,
) -> Element {
    let Cell { entry, look, feed } = cell;
    let tile = look.tile;
    let picture = picture(&entry, &look, feed, thumbnailing);
    let name = text(entry.name().to_string())
        .caption()
        .no_wrap()
        .width(tile);
    let meta = text(wording::meta(&entry))
        .caption()
        .muted()
        .no_wrap()
        .width(tile);
    let mismatched = matches!(entry.fit(), Fit::Mismatch(_));
    let face = column! {
        picture;
        name;
        meta;
    }
    .gap(Space::TIGHT)
    .width(tile)
    .opacity(if mismatched { MISMATCHED } else { 1.0 });

    let chosen = entry.id();
    let lifted = Arc::clone(&entry);
    let symbol = wording::symbol(entry.shelf());
    let lifting = lift(
        face,
        Arc::clone(entry.path()),
        Arc::clone(entry.name()),
        symbol,
    )
    .on_press(reach.act(move |shelving| shelving.browser_mut().select(chosen)))
    .on_lift(reach.act(move |shelving| shelving.browser_mut().lift(Arc::clone(&lifted))));

    let hovered = Arc::clone(&entry);
    let hovering = container(lifting)
        .on_hover(reach.act_with(move |shelving, inside: bool| {
            hover(shelving, reach, &hovered, inside);
        }))
        .tooltip(wording::summary(&entry));

    let loaded = Arc::clone(&entry);
    let revealed = Arc::clone(entry.path());
    context_area(hovering, move || {
        let loaded = Arc::clone(&loaded);
        let revealed = Arc::clone(&revealed);
        vec![
            menu_item(tr!("animate.library.load"))
                .enabled(loaded.is_usable())
                .on_press(reach.act(move |shelving| shelving.take(Arc::clone(&loaded)))),
            menu_item(tr!("animate.library.reveal"))
                .on_press(reach.act(move |shelving| shelving.reveal(&revealed))),
        ]
    })
    .build()
}

fn hover(
    shelving: &mut dyn Shelving,
    reach: Reach<dyn Shelving>,
    entry: &Arc<Entry>,
    inside: bool,
) {
    skip_view();
    if !inside {
        cancel(PREVIEW_KEY);
        if shelving.browser().previewed() == Some(entry.id()) {
            shelving.preview(None);
        }
        return;
    }
    if !entry.is_usable() || !matches!(entry.shelf(), Shelf::Motions | Shelf::Cameras) {
        return;
    }
    let started = Arc::clone(entry);
    after_keyed(
        PREVIEW_KEY,
        PREVIEW_DELAY,
        reach.act(move |shelving| shelving.preview(Some(Arc::clone(&started)))),
    );
}

fn picture(
    entry: &Arc<Entry>,
    cell: &Look,
    feed: Option<Feed>,
    thumbnailing: Option<&Thumbnailing>,
) -> Element {
    let Look {
        tile,
        variant,
        selected,
    } = *cell;
    let palette = palette();
    let content = match feed.filter(|feed| feed.size().is_some()) {
        Some(feed) => image(ImageSource::Feed(feed))
            .fit(ImageFit::Cover)
            .fill()
            .any(),
        None => match thumbnail::shown(entry, variant, thumbnailing) {
            Shown::Picture(path) => image(ImageSource::file(path.as_path()))
                .fit(ImageFit::Cover)
                .fill()
                .any(),
            Shown::Waiting => container(spinner()).center().fill().any(),
            Shown::Bare => container(
                icon(wording::symbol(entry.shelf()))
                    .extent(Length::px(tile * BARE_SHARE))
                    .color(palette.text_muted),
            )
            .center()
            .fill()
            .any(),
        },
    };
    let frame = match selected {
        true => palette.accent,
        false => Color::TRANSPARENT,
    };
    container(content)
        .background(palette.surface_variant)
        .border(SELECTED_BORDER, frame)
        .radius(Corner::Item)
        .clip_content()
        .size(tile, tile)
        .any()
}
